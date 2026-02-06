use bevy_ecs::{
    resource::Resource,
    system::{Res, ResMut},
};
use common::{
    blocks::block_type::{BlockContent, BlockType},
    default_resources::DEFAULT_RESOURCES,
    utils::{calculate_hash, split_resource_path},
};
use network::messages::ResurceScheme;
use std::{collections::BTreeMap, fs, path::PathBuf};

use super::{plugin_container::PluginContainer, resources_archive::ResourcesArchive, server_settings::ServerSettings};
use crate::{launch_settings::LaunchSettings, network::runtime_plugin::RuntimePlugin};

#[derive(Resource, Default)]
pub struct PluginsManager {
    plugins: BTreeMap<String, PluginContainer>,
    resources_archive: Option<ResourcesArchive>,
}

impl PluginsManager {
    pub fn get_resources_archive(&self) -> &ResourcesArchive {
        &self.resources_archive.as_ref().expect("GET_RESOURCES_ARCHIVE: resources_archive is not set")
    }

    pub fn rescan_plugins(&mut self, path: PathBuf, server_settings: &mut ServerSettings) -> Result<(), String> {
        self.unload_all_plugins();

        let mut resources_archive = ResourcesArchive::default();
        let path_str = path.into_os_string().into_string().unwrap();
        log::info!(target: "resources", "▼ Rescan plugins folders inside: &e{}", path_str);

        let resource_paths = match fs::read_dir(path_str.clone()) {
            Ok(p) => p,
            Err(e) => {
                return Err(format!("read directory &e\"{}\"&r error: &c{}", path_str, e));
            }
        };

        for resource_path in resource_paths {
            let resource_path = resource_path.unwrap().path();

            let mut manifest_path = resource_path.clone();
            manifest_path.push("manifest.yml");
            if !manifest_path.exists() {
                continue;
            }

            let plugin = match PluginContainer::from_manifest(resource_path.clone()) {
                Ok(i) => i,
                Err(e) => {
                    return Err(format!("Resource &e{}: \n&c{}", resource_path.display().to_string(), e));
                }
            };
            let resource_slug = plugin.get_slug().clone();

            if self.plugins.contains_key(&resource_slug) {
                return Err(format!(
                    "&cresource &4\"{}\"&c slug &4\"{}\"&c already exists",
                    resource_path.display().to_string(),
                    resource_slug
                ));
            }

            let blocks = plugin.get_blocks();
            for block_type in blocks.iter() {
                server_settings.add_block(block_type.clone());
            }

            log::info!(
                target: "resources",
                " □ Plugin &2\"{}\"&r loaded;&7 Title:&8\"{}\" &7v:&8\"{}\" &7Author:&8\"{}\" &7Scripts:&8{} &7Media:&8{} &7Blocks:&8{}",
                plugin.get_slug(),
                plugin.get_title(),
                plugin.get_version(),
                plugin.get_autor(),
                plugin.get_scripts_count(),
                plugin.get_media_count(),
                blocks.len(),
            );

            let mut scheme = ResurceScheme {
                slug: plugin.get_slug().clone(),
                scripts: Default::default(),
                media: Default::default(),
            };
            for (script_slug, scripts_data) in plugin.iter_scripts() {
                let hash = calculate_hash(&scripts_data);
                resources_archive.add_entry(hash.to_string(), scripts_data.as_bytes().to_vec());
                scheme.scripts.insert(hash.to_string(), script_slug.clone());
            }

            for (media_slug, media_data) in plugin.iter_media() {
                let hash = calculate_hash(&media_data);
                resources_archive.add_entry(hash.to_string(), media_data.clone());
                scheme.media.insert(hash.to_string(), media_slug.clone());
            }
            resources_archive.add_resource_scheme(scheme);

            self.add_plugin(plugin.get_slug().clone(), plugin);

            if let Err(e) = self.validate_blocks(&blocks) {
                return Err(format!("resource &6\"{}\"&r: {}", resource_slug, e));
            }
        }
        resources_archive.finalize();

        self.resources_archive = Some(resources_archive);
        log::info!(target: "resources", "All plugins have been successfully loaded: {}", self.plugins.len());
        Ok(())
    }

    pub fn has_media(&self, path: &String) -> Result<bool, String> {
        if DEFAULT_RESOURCES.contains(&path.as_str()) {
            return Ok(true);
        }

        let Some((res_slug, res_path)) = split_resource_path(path) else {
            return Err(format!("cannot split path \"{}\"", path));
        };

        let Some(resource) = self.plugins.get(&res_slug) else {
            return Err(format!("plugin \"{}\" not found", res_slug));
        };

        if !resource.has_media(&res_path) {
            return Err(format!(
                "plugin \"{}\" doesn't contain media \"{}\"; total count: {}",
                res_slug,
                res_path,
                resource.media.len()
            ));
        }
        return Ok(true);
    }

    pub fn validate_blocks(&self, blocks: &Vec<BlockType>) -> Result<(), String> {
        for block_type in blocks.iter() {
            match block_type.get_block_content() {
                BlockContent::Texture {
                    texture,
                    side_texture,
                    side_overlay,
                    bottom_texture,
                    ..
                } => {
                    if let Err(e) = self.has_media(texture) {
                        return Err(format!(
                            "&cblock &4\"{}\" &ctexture not found: &4\"{}\" &7({})",
                            block_type.get_slug(),
                            texture,
                            e,
                        ));
                    }
                    if side_texture.is_some() {
                        if let Err(e) = self.has_media(&side_texture.as_ref().unwrap()) {
                            return Err(format!(
                                "&cblock &4\"{}\" &cside_texture not found: &4\"{}\" &7({})",
                                block_type.get_slug(),
                                side_texture.as_ref().unwrap(),
                                e,
                            ));
                        }
                    }
                    if side_overlay.is_some() {
                        if let Err(e) = self.has_media(&side_overlay.as_ref().unwrap()) {
                            return Err(format!(
                                "&cblock &4\"{}\" &cside_overlay not found: &4\"{}\" &7({})",
                                block_type.get_slug(),
                                side_overlay.as_ref().unwrap(),
                                e,
                            ));
                        }
                    }
                    if bottom_texture.is_some() {
                        if let Err(e) = self.has_media(&bottom_texture.as_ref().unwrap()) {
                            return Err(format!(
                                "&cblock &4\"{}\" &cbottom_texture not found: &4\"{}\" &7({})",
                                block_type.get_slug(),
                                bottom_texture.as_ref().unwrap(),
                                e,
                            ));
                        }
                    }
                }
                BlockContent::ModelCube { model, .. } => {
                    if let Err(e) = self.has_media(model) {
                        return Err(format!(
                            "&cblock &4\"{}\" &cmodel not found: &4\"{}\" &7({})",
                            block_type.get_slug(),
                            model,
                            e,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn add_plugin(&mut self, slug: String, plugin: PluginContainer) {
        self.plugins.insert(slug, plugin);
    }

    pub fn unload_all_plugins(&mut self) {
        for (slug, plugin) in self.plugins.iter_mut() {
            if let Err(e) = plugin.unload() {
                log::warn!(target: "resources", "Error unloading plugin \"{}\": {}", slug, e);
            }
        }
        self.plugins.clear();
    }
}

pub(crate) fn rescan_plugins(
    mut plugins_manager: ResMut<PluginsManager>,
    launch_settings: Res<LaunchSettings>,
    mut server_settings: ResMut<ServerSettings>,
) {
    if let Err(e) = plugins_manager.rescan_plugins(launch_settings.get_plugins_path(), &mut *server_settings) {
        log::error!(target: "resources", "&cPlugins loading error:");
        log::error!(target: "resources", "{}", e);
        RuntimePlugin::stop();
        return;
    }
}
