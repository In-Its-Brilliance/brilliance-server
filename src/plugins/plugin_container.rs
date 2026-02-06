use common::blocks::block_type::{BlockContent, BlockType, BlockTypeManifest};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::plugins::server_plugin::plugin_instance::PluginInstance;

const ALLOWED_FILES_EXT: &'static [&'static str] = &[".png", ".glb"];

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ResourceManifest {
    pub slug: String,
    pub title: Option<String>,
    pub autor: Option<String>,
    pub version: Option<String>,
    pub client_scripts: Option<Vec<String>>,
    pub media: Option<Vec<String>>,

    pub blocks: Option<Vec<BlockTypeManifest>>,
}

pub struct PluginContainer {
    slug: String,
    title: String,
    autor: Option<String>,
    version: Option<String>,
    scripts: BTreeMap<String, String>,
    pub(crate) media: BTreeMap<String, Vec<u8>>,

    blocks: Vec<BlockType>,

    plugin: Option<PluginInstance>,
}

impl PluginContainer {
    pub fn get_slug(&self) -> &String {
        &self.slug
    }
    pub fn get_title(&self) -> &String {
        &self.title
    }
    pub fn get_autor(&self) -> String {
        match &self.autor {
            Some(s) => s.clone(),
            None => "-".to_string(),
        }
    }
    pub fn get_version(&self) -> String {
        match &self.version {
            Some(s) => s.clone(),
            None => "-".to_string(),
        }
    }
    pub fn get_scripts_count(&self) -> usize {
        self.scripts.len()
    }
    pub fn get_media_count(&self) -> usize {
        self.media.len()
    }

    pub fn find_plugin_wasm(plugin_dir: &Path) -> Result<Option<PathBuf>, String> {
        let entries =
            fs::read_dir(plugin_dir).map_err(|e| format!("cannot read dir \"{}\": {}", plugin_dir.display(), e))?;

        let mut wasm_file: Option<PathBuf> = None;

        for entry in entries {
            let path = entry.map_err(|e| format!("read entry error: {}", e))?.path();

            if path.extension().and_then(|e| e.to_str()) == Some("wasm") {
                if wasm_file.is_some() {
                    return Err(format!("plugin \"{}\" has multiple .wasm files", plugin_dir.display()));
                }
                wasm_file = Some(path);
            }
        }

        Ok(wasm_file)
    }

    pub fn from_manifest(resource_path: PathBuf) -> Result<Self, String> {
        let mut manifest_path = resource_path.clone();
        manifest_path.push("manifest.yml");

        log::debug!(target: "resources", "Start loading &e\"{}\"", manifest_path.display());

        let manifest_data = match std::fs::read_to_string(manifest_path.clone()) {
            Ok(d) => d,
            Err(e) => {
                return Err(format!("file error: &c{}", e));
            }
        };

        let manifest_result: Result<ResourceManifest, serde_yaml::Error> = serde_yaml::from_str(&manifest_data);
        let manifest = match manifest_result {
            Ok(m) => m,
            Err(e) => {
                return Err(format!("error with parse manifest yaml: &c{}", e));
            }
        };

        let title = match &manifest.title {
            Some(t) => t.clone(),
            None => manifest.slug.clone(),
        };

        let mut inst = Self {
            slug: manifest.slug.clone(),
            title: title,
            autor: manifest.autor.clone(),
            version: manifest.version.clone(),
            scripts: Default::default(),
            media: Default::default(),
            blocks: Default::default(),
            plugin: Default::default(),
        };

        if let Some(wasm_path) = Self::find_plugin_wasm(&resource_path)? {
            let mut plugin_wasm = match PluginInstance::new(&wasm_path) {
                Ok(w) => w,
                Err(e) => return Err(format!("WASM plugin {:?}\n&4Error: &c{}", wasm_path.display(), e)),
            };
            if let Err(e) = plugin_wasm.call_on_enable(&manifest.slug.clone()) {
                return Err(format!("WASM plugin {:?}\nOn enable error: &c{}", wasm_path.display(), e));
            }
            inst.plugin = Some(plugin_wasm);
        }

        let manifest_blocks = match manifest.blocks {
            Some(b) => b,
            None => Default::default(),
        };
        for block in manifest_blocks.iter() {
            let category = match block.category.clone() {
                Some(c) => c,
                None => inst.slug.clone(),
            };
            let mut b = BlockType::new(block.block_content.clone()).category(category);
            if let Some(slug) = block.slug.as_ref() {
                b = b.set_slug(slug.clone());
            }
            b = b.visibility(block.voxel_visibility);
            inst.blocks.push(b);
        }

        if let Some(client_scripts) = &manifest.client_scripts {
            for client_script in client_scripts.iter() {
                let mut script_path = resource_path.clone();
                script_path.push(client_script);

                let data = match std::fs::read_to_string(script_path) {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!(target: "resources", "□ script file &e\"{}\"&r error: &c{:?}", client_script, e);
                        continue;
                    }
                };
                inst.add_script(client_script.clone(), data);
            }
        }
        if let Some(media_list) = &manifest.media {
            for media in media_list.iter() {
                if !Self::is_media_allowed(&media) {
                    return Err(format!("file extension is not supported &c{}", media));
                }

                let mut media_path = resource_path.clone();
                media_path.push(media);

                let data = match std::fs::read(media_path.clone()) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!(
                            "media content file &e\"{}\"&r error: &c{:?}",
                            media_path.display(),
                            e
                        ));
                    }
                };
                inst.add_media(media.clone(), data);
            }
        }

        Ok(inst)
    }

    pub fn has_media(&self, slug: &String) -> bool {
        self.media.contains_key(slug)
    }

    pub fn iter_scripts(&self) -> std::collections::btree_map::Iter<'_, String, String> {
        self.scripts.iter()
    }

    pub fn iter_media(&self) -> std::collections::btree_map::Iter<'_, String, Vec<u8>> {
        self.media.iter()
    }

    pub(crate) fn get_blocks(&self) -> Vec<BlockType> {
        let mut blocks = self.blocks.clone();

        for block_type in blocks.iter_mut() {
            match block_type.get_block_content_mut() {
                BlockContent::Texture {
                    texture,
                    side_texture,
                    side_overlay,
                    bottom_texture,
                    ..
                } => {
                    *texture = self.local_to_global_path(&texture);
                    if let Some(texture) = side_texture {
                        *texture = self.local_to_global_path(texture);
                    }
                    if let Some(texture) = side_overlay {
                        *texture = self.local_to_global_path(texture);
                    }
                    if let Some(texture) = bottom_texture {
                        *texture = self.local_to_global_path(texture);
                    }
                }
                BlockContent::ModelCube { model, .. } => {
                    *model = self.local_to_global_path(model);
                }
            }
        }
        blocks
    }

    pub fn local_to_global_path(&self, path: &String) -> String {
        format!("{}://{}", self.get_slug(), path)
    }

    fn is_media_allowed(media: &str) -> bool {
        for ext in ALLOWED_FILES_EXT.iter() {
            if media.ends_with(ext) {
                return true;
            }
        }
        return false;
    }

    pub fn add_script(&mut self, slug: String, data: String) {
        self.scripts.insert(slug, data);
    }

    pub fn add_media(&mut self, slug: String, data: Vec<u8>) {
        self.media.insert(slug, data);
    }

    pub fn unload(&mut self) -> Result<(), String> {
        if let Some(ref mut wasm_instance) = self.plugin {
            wasm_instance.call_on_disable(&self.slug)?;
        }
        self.plugin = None;
        Ok(())
    }
}
