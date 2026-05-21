use crate::{
    clients::{client::WorldEntity, clients_container::ClientsContainer},
    inventory::{
        commands::{close_inventory, get_or_create_inventory, open_inventory},
        inventory_manager::InventoryManager,
    },
    items_manager::{ItemInfo as ServerItemInfo, ItemType as ServerItemType},
    network::sync_world_change::sync_world_block_change,
    storage::storage_manager::StorageManager,
    worlds::worlds_manager::WorldsManager,
};
use common::{
    chunks::{
        block_position::BlockPosition,
        chunk_data::BlockDataInfo,
    },
    inventory::item::{Item, ItemKind},
    plugin_api::items_manager::{ItemInfo as ApiItemInfo, ItemType as ApiItemType},
    plugin_api::inventory::OpenInventoryRequest,
    server_storage::taits::{IServerStorage, PlayerData},
    utils::debug::SmartRwLock,
};
use extism::*;
use serde::Deserialize;
use serde_json;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

pub type SharedHostContext = Arc<parking_lot::Mutex<HostContext>>;

static WORLDS_MANAGER_BRIDGE: OnceLock<Arc<SmartRwLock<WorldsManager>>> = OnceLock::new();
static SERVER_STORAGE_BRIDGE: OnceLock<Arc<SmartRwLock<StorageManager>>> = OnceLock::new();
static CLIENTS_CONTAINER_BRIDGE: OnceLock<Arc<SmartRwLock<ClientsContainer>>> = OnceLock::new();
static INVENTORY_MANAGER_BRIDGE: OnceLock<Arc<SmartRwLock<InventoryManager>>> = OnceLock::new();
static ITEMS_MANAGER_BRIDGE: OnceLock<Arc<SmartRwLock<crate::items_manager::items_manager::ItemsManager>>> = OnceLock::new();
static PLUGINS_MANAGER_BRIDGE: OnceLock<usize> = OnceLock::new();

#[derive(Default)]
pub struct HostContext {
    plugin_slug: String,
    plugin_root_path: PathBuf,
    world_generators: Vec<String>,
    has_on_chunk_generate: bool,
}

impl HostContext {
    pub fn create(plugin_slug: String, plugin_root_path: PathBuf) -> Self {
        Self {
            plugin_slug,
            plugin_root_path,
            ..Default::default()
        }
    }

    pub fn get_plugin_slug(&self) -> &String {
        &self.plugin_slug
    }

    pub fn get_plugin_root_path(&self) -> &PathBuf {
        &self.plugin_root_path
    }

    pub fn get_world_generators(&self) -> &Vec<String> {
        &self.world_generators
    }

    pub(crate) fn set_has_on_chunk_generate(&mut self) {
        self.has_on_chunk_generate = true;
    }
}

pub fn set_worlds_manager_bridge(worlds_manager: Arc<SmartRwLock<WorldsManager>>) {
    let _ = WORLDS_MANAGER_BRIDGE.set(worlds_manager);
}

pub fn set_server_storage_bridge(server_storage: Arc<SmartRwLock<StorageManager>>) {
    let _ = SERVER_STORAGE_BRIDGE.set(server_storage);
}

pub fn set_clients_container_bridge(clients_container: Arc<SmartRwLock<ClientsContainer>>) {
    let _ = CLIENTS_CONTAINER_BRIDGE.set(clients_container);
}

pub fn set_inventory_manager_bridge(
    inventory_manager: Arc<SmartRwLock<crate::inventory::inventory_manager::InventoryManager>>,
) {
    let _ = INVENTORY_MANAGER_BRIDGE.set(inventory_manager);
}

pub fn set_items_manager_bridge(items_manager: Arc<SmartRwLock<crate::items_manager::items_manager::ItemsManager>>) {
    let _ = ITEMS_MANAGER_BRIDGE.set(items_manager);
}

pub fn set_plugins_manager_bridge(plugins_manager: &crate::plugins::plugins_manager::PluginsManager) {
    let _ = PLUGINS_MANAGER_BRIDGE.set(plugins_manager as *const _ as usize);
}

fn resolve_plugin_path(root_path: &Path, relative_path: &str) -> Result<PathBuf, Error> {
    let mut sanitized = PathBuf::new();
    for component in Path::new(relative_path).components() {
        match component {
            std::path::Component::Normal(part) => sanitized.push(part),
            std::path::Component::CurDir => {}
            _ => {
                return Err(Error::msg(format!("Invalid plugin path \"{}\"", relative_path)));
            }
        }
    }

    let full_path = root_path.join(sanitized);
    let canonical_root = fs::canonicalize(root_path)
        .map_err(|e| Error::msg(format!("Failed to canonicalize plugin root \"{}\": {}", root_path.display(), e)))?;
    let canonical_full = fs::canonicalize(&full_path)
        .map_err(|e| Error::msg(format!("Failed to access plugin path \"{}\": {}", full_path.display(), e)))?;

    if !canonical_full.starts_with(&canonical_root) {
        return Err(Error::msg(format!("Plugin path \"{}\" is outside plugin root", relative_path)));
    }

    Ok(canonical_full)
}

fn ensure_not_hardlinked(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::metadata(path)
            .map_err(|e| Error::msg(format!("Failed to read metadata for \"{}\": {}", path.display(), e)))?;
        if metadata.nlink() > 1 {
            return Err(Error::msg(format!(
                "Hardlinked file access is not allowed: \"{}\"",
                path.display()
            )));
        }
    }

    Ok(())
}

fn list_dir_entries(path: &Path) -> Result<Vec<String>, Error> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| Error::msg(format!("Failed to read dir \"{}\": {}", path.display(), e)))? {
        let entry = entry.map_err(|e| Error::msg(format!("Failed to read dir entry: {}", e)))?;
        let file_name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::msg(format!("Invalid unicode path in \"{}\"", path.display())))?;
        entries.push(file_name);
    }
    entries.sort();
    Ok(entries)
}

fn emit_inventory_change_to_client(
    client: &crate::clients::client::Client,
    inventory_type: common::inventory::inventory::InventoryType,
    slot: usize,
    item: Option<&Item>,
    items_manager: &Arc<SmartRwLock<crate::items_manager::items_manager::ItemsManager>>,
) {
    let change = network::messages::InventorySlotChange {
        slot,
        item: item.map(|item| items_manager.read().to_client_item(item)),
    };
    client.send_message(
        network::messages::NetworkMessageType::ReliableOrdered,
        &network::messages::ServerMessages::InventoryStream(network::messages::InventoryStream::UpdateSlots {
            inventory_type,
            changes: vec![change],
        }),
    );
}

fn emit_inventory_change_to_watchers(
    clients: &Arc<SmartRwLock<ClientsContainer>>,
    inventory_manager: &InventoryManager,
    inventory_id: u64,
    slot: usize,
    item: Option<&Item>,
    items_manager: &Arc<SmartRwLock<crate::items_manager::items_manager::ItemsManager>>,
) {
    let Some(watchers) = inventory_manager.state().get_inventory_watchers(&inventory_id) else {
        return;
    };

    let clients_guard = clients.read();
    let change = network::messages::InventorySlotChange {
        slot,
        item: item.map(|item| items_manager.read().to_client_item(item)),
    };
    let stream = network::messages::ServerMessages::InventoryStream(network::messages::InventoryStream::UpdateSlots {
        inventory_type: common::inventory::inventory::InventoryType::WorldInventory(inventory_id),
        changes: vec![change],
    });

    for watcher in watchers {
        let Some(watcher_client) = clients_guard.iter().find_map(|(_, candidate)| {
            let Some(world_entity) = candidate.get_world_entity() else {
                return None;
            };
            (world_entity.get_entity() == *watcher).then_some(candidate)
        }) else {
            continue;
        };
        watcher_client.send_message(network::messages::NetworkMessageType::ReliableOrdered, &stream);
    }
}

fn get_worlds_manager_bridge() -> Option<Arc<SmartRwLock<WorldsManager>>> {
    WORLDS_MANAGER_BRIDGE.get().cloned()
}

fn get_server_storage_bridge() -> Option<Arc<SmartRwLock<StorageManager>>> {
    SERVER_STORAGE_BRIDGE.get().cloned()
}

fn get_clients_container_bridge() -> Option<Arc<SmartRwLock<ClientsContainer>>> {
    CLIENTS_CONTAINER_BRIDGE.get().cloned()
}

fn get_inventory_manager_bridge() -> Option<Arc<SmartRwLock<crate::inventory::inventory_manager::InventoryManager>>> {
    INVENTORY_MANAGER_BRIDGE.get().cloned()
}

fn get_items_manager_bridge() -> Option<Arc<SmartRwLock<crate::items_manager::items_manager::ItemsManager>>> {
    ITEMS_MANAGER_BRIDGE.get().cloned()
}

fn get_plugins_manager_bridge() -> Option<&'static crate::plugins::plugins_manager::PluginsManager> {
    let ptr = *PLUGINS_MANAGER_BRIDGE.get()?;
    Some(unsafe { &*(ptr as *const crate::plugins::plugins_manager::PluginsManager) })
}

pub fn register_world_generator_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let name: String = plugin.memory_get_val(&inputs[0])?;
    let inner = user_data.get()?;
    let inner = inner.lock().unwrap();
    let mut ctx = inner.lock();

    if !ctx.has_on_chunk_generate {
        return Err(Error::msg(
            "Plugin must implement event for ChunkGenerateEvent to register world generator!",
        ));
    }
    log::debug!(target: "plugin", "[Host] Plugin '{}' registered generator: {}", ctx.plugin_slug, name);
    ctx.world_generators.push(name);
    plugin.memory_set_val(&mut outputs[0], "")?;
    Ok(())
}

pub fn get_plugin_slug_raw(
    plugin: &mut CurrentPlugin,
    _inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let inner = user_data.get()?;
    let inner = inner.lock().unwrap();
    let ctx = inner.lock();
    plugin.memory_set_val(&mut outputs[0], ctx.get_plugin_slug())?;
    Ok(())
}

pub fn read_dir_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let path: String = plugin.memory_get_val(&inputs[0])?;
    let inner = user_data.get()?;
    let inner = inner.lock().unwrap();
    let ctx = inner.lock();
    let resolved = resolve_plugin_path(ctx.get_plugin_root_path(), &path)?;
    let entries = list_dir_entries(&resolved)?;
    let entries_json =
        serde_json::to_string(&entries).map_err(|e| Error::msg(format!("Serialize dir list failed: {}", e)))?;
    plugin.memory_set_val(&mut outputs[0], entries_json)?;
    Ok(())
}

pub fn read_file_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let path: String = plugin.memory_get_val(&inputs[0])?;
    let inner = user_data.get()?;
    let inner = inner.lock().unwrap();
    let ctx = inner.lock();
    let resolved = resolve_plugin_path(ctx.get_plugin_root_path(), &path)?;
    ensure_not_hardlinked(&resolved)?;
    let data = fs::read_to_string(&resolved)
        .map_err(|e| Error::msg(format!("Failed to read file \"{}\": {}", resolved.display(), e)))?;
    plugin.memory_set_val(&mut outputs[0], data)?;
    Ok(())
}

pub fn add_inventory_item_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let inventory_id: u64 = plugin.memory_get_val(&inputs[0])?;
    let item_json: String = plugin.memory_get_val(&inputs[1])?;
    let item: Item = serde_json::from_str(&item_json)
        .map_err(|e| Error::msg(format!("Invalid inventory item json: {}", e)))?;

    if let ItemKind::CustomItem(slug) = item.get_item_kind() {
        let items_manager =
            get_items_manager_bridge().ok_or_else(|| Error::msg("ItemsManager bridge is not initialized"))?;
        if !items_manager.read().has_item(slug) {
            plugin.memory_set_val(&mut outputs[0], "not_found")?;
            return Ok(());
        }
    }

    let clients = get_clients_container_bridge().ok_or_else(|| Error::msg("ClientsContainer bridge is not initialized"))?;
    let inventory_manager =
        get_inventory_manager_bridge().ok_or_else(|| Error::msg("InventoryManager bridge is not initialized"))?;
    let worlds_manager =
        get_worlds_manager_bridge().ok_or_else(|| Error::msg("WorldsManager bridge is not initialized"))?;
    let items_manager =
        get_items_manager_bridge().ok_or_else(|| Error::msg("ItemsManager bridge is not initialized"))?;

    let clients_guard = clients.read();
    for (_client_id, client) in clients_guard.iter() {
        let is_target_inventory = client
            .get_player_data()
            .map(|player_data| player_data.get_inventory().get_id() == inventory_id)
            .unwrap_or(false);
        if !is_target_inventory {
            continue;
        }

        let result = client.with_player_data_mut(|player_data| {
            player_data.get_inventory_mut().add_item(item.clone(), |slot, updated_item| {
                emit_inventory_change_to_client(
                    client,
                    common::inventory::inventory::InventoryType::PlayerPersonal,
                    slot,
                    updated_item,
                    &items_manager,
                );
            })
        });
        let Some(Ok(())) = result else {
            plugin.memory_set_val(&mut outputs[0], "full")?;
            return Ok(());
        };
        plugin.memory_set_val(&mut outputs[0], "ok")?;
        return Ok(());
    }
    drop(clients_guard);

    let Some(location) = inventory_manager.read().state().get_inventory_location(&inventory_id).cloned() else {
        plugin.memory_set_val(&mut outputs[0], "not_found")?;
        return Ok(());
    };

    let worlds_guard = worlds_manager.write();
    let Some(world_manager) = worlds_guard.get_world_manager_mut(location.get_world_slug()) else {
        plugin.memory_set_val(&mut outputs[0], "not_found")?;
        return Ok(());
    };
    let Some(chunk_column_arc) = world_manager.get_chunks_map().get_chunk_column_arc(location.get_chunk_position()) else {
        plugin.memory_set_val(&mut outputs[0], "not_found")?;
        return Ok(());
    };
    let mut chunk_column = chunk_column_arc.write();
    let Some(block_inventory) = chunk_column.get_chunk_storage_mut().get_inventory_mut(inventory_id) else {
        plugin.memory_set_val(&mut outputs[0], "not_found")?;
        return Ok(());
    };

    let Ok(()) = block_inventory.get_inventory_mut().add_item(item.clone(), |slot, updated_item| {
        emit_inventory_change_to_watchers(
            &clients,
            &inventory_manager.read(),
            inventory_id,
            slot,
            updated_item,
            &items_manager,
        );
    }) else {
        plugin.memory_set_val(&mut outputs[0], "full")?;
        return Ok(());
    };

    plugin.memory_set_val(&mut outputs[0], "ok")?;
    Ok(())
}

pub fn has_world_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let slug: String = plugin.memory_get_val(&inputs[0])?;
    let worlds_manager =
        get_worlds_manager_bridge().ok_or_else(|| Error::msg("WorldsManager bridge is not initialized"))?;
    let has_world = worlds_manager.read().has_world_with_slug(&slug);
    plugin.memory_set_val(&mut outputs[0], if has_world { "true" } else { "false" })?;
    Ok(())
}

pub fn create_world_raw(
    _plugin: &mut CurrentPlugin,
    _inputs: &[Val],
    _outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    Err(Error::msg("create_world_raw is not implemented in host bridge"))
}

pub fn get_player_world_slug_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let client_id: u64 = plugin.memory_get_val(&inputs[0])?;
    let clients =
        get_clients_container_bridge().ok_or_else(|| Error::msg("ClientsContainer bridge is not initialized"))?;
    let world_slug = clients
        .read()
        .get(&client_id)
        .and_then(|client| client.get_world_entity())
        .map(|world_entity: WorldEntity| world_entity.get_world_slug().clone())
        .unwrap_or_default();
    plugin.memory_set_val(&mut outputs[0], world_slug)?;
    Ok(())
}

pub fn get_player_inventory_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let client_id: u64 = plugin.memory_get_val(&inputs[0])?;
    let clients =
        get_clients_container_bridge().ok_or_else(|| Error::msg("ClientsContainer bridge is not initialized"))?;
    let inventory_id = clients
        .read()
        .get(&client_id)
        .and_then(|client| client.get_player_data())
        .map(|player_data| player_data.get_inventory().get_id().to_string())
        .unwrap_or_default();
    plugin.memory_set_val(&mut outputs[0], inventory_id)?;
    Ok(())
}

pub fn add_item_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let item_json: String = plugin.memory_get_val(&inputs[0])?;
    let item: ApiItemInfo = serde_json::from_str(&item_json)
        .map_err(|e| Error::msg(format!("Invalid item info json: {}", e)))?;

    let server_item_type = match item.get_item_type() {
        ApiItemType::Armor {
            body_part,
            icon,
            model,
        } => ServerItemType::armor(body_part.clone(), icon.clone(), model.clone()),
        ApiItemType::Weapon {
            weapon_kind,
            icon,
            model,
        } => ServerItemType::weapon(weapon_kind.clone(), icon.clone(), model.clone()),
        ApiItemType::Other { icon } => ServerItemType::other(icon.clone()),
    };

    let server_item = ServerItemInfo::create(
        item.get_slug().clone(),
        server_item_type,
        item.get_title().clone(),
        item.get_description().clone(),
        item.get_max_stack_size(),
    );

    let items_manager =
        get_items_manager_bridge().ok_or_else(|| Error::msg("ItemsManager bridge is not initialized"))?;
    let plugins_manager =
        get_plugins_manager_bridge().ok_or_else(|| Error::msg("PluginsManager bridge is not initialized"))?;
    items_manager
        .write()
        .add_item(plugins_manager, server_item)
        .map_err(Error::msg)?;

    plugin.memory_set_val(&mut outputs[0], "")?;
    Ok(())
}

pub fn get_or_create_player_data_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let username: String = plugin.memory_get_val(&inputs[0])?;
    let server_storage =
        get_server_storage_bridge().ok_or_else(|| Error::msg("ServerStorage bridge is not initialized"))?;

    let player_data = server_storage
        .read()
        .read_server_storage()
        .get_or_create_player_data(username)
        .map_err(Error::msg)?;
    let player_data_json =
        serde_json::to_string(&player_data).map_err(|e| Error::msg(format!("Serialize player data failed: {}", e)))?;

    plugin.memory_set_val(&mut outputs[0], player_data_json)?;
    Ok(())
}

pub fn save_player_data_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let player_data_json: String = plugin.memory_get_val(&inputs[0])?;
    let player_data: PlayerData =
        serde_json::from_str(&player_data_json).map_err(|e| Error::msg(format!("Invalid player data json: {}", e)))?;
    let server_storage =
        get_server_storage_bridge().ok_or_else(|| Error::msg("ServerStorage bridge is not initialized"))?;

    let player_id = server_storage
        .read()
        .read_server_storage()
        .save_player_data(&player_data)
        .map_err(Error::msg)?;

    plugin.memory_set_val(&mut outputs[0], player_id.to_string())?;
    Ok(())
}

pub fn edit_world_block_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let world_slug: String = plugin.memory_get_val(&inputs[0])?;
    let position_json: String = plugin.memory_get_val(&inputs[1])?;
    let new_block_info_json: String = plugin.memory_get_val(&inputs[2])?;

    #[derive(Deserialize)]
    struct BlockPositionJson {
        x: f32,
        y: f32,
        z: f32,
    }

    let position_json: BlockPositionJson =
        serde_json::from_str(&position_json).map_err(|e| Error::msg(format!("Invalid block position json: {}", e)))?;
    let position = BlockPosition::new(
        position_json.x.floor() as i64,
        position_json.y.floor() as i64,
        position_json.z.floor() as i64,
    );
    let new_block_info: Option<BlockDataInfo> = serde_json::from_str(&new_block_info_json)
        .map_err(|e| Error::msg(format!("Invalid block data json: {}", e)))?;

    let worlds_manager =
        get_worlds_manager_bridge().ok_or_else(|| Error::msg("WorldsManager bridge is not initialized"))?;
    let worlds_manager = worlds_manager.write();
    let Some(world_manager) = worlds_manager.get_world_manager_mut(&world_slug) else {
        return Err(Error::msg(format!("World \"{}\" not found", world_slug)));
    };

    world_manager
        .get_chunks_map()
        .edit_block(position.clone(), new_block_info.clone())
        .map_err(|e| Error::msg(format!("Edit block failed: {}", e)))?;

    sync_world_block_change(&*world_manager, position, new_block_info);
    plugin.memory_set_val(&mut outputs[0], "")?;
    Ok(())
}

pub fn get_or_create_inventory_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let world_slug: String = plugin.memory_get_val(&inputs[0])?;
    let position_json: String = plugin.memory_get_val(&inputs[1])?;
    let slots_count: u64 = plugin.memory_get_val(&inputs[2])?;

    let position: BlockPosition =
        serde_json::from_str(&position_json).map_err(|e| Error::msg(format!("Invalid block position json: {}", e)))?;
    let worlds_manager =
        get_worlds_manager_bridge().ok_or_else(|| Error::msg("WorldsManager bridge is not initialized"))?;
    let inventory_manager =
        get_inventory_manager_bridge().ok_or_else(|| Error::msg("InventoryManager bridge is not initialized"))?;
    let inventory_id = get_or_create_inventory(
        world_slug,
        position,
        slots_count as usize,
        &worlds_manager,
        &inventory_manager,
    )
    .map_err(Error::msg)?;

    plugin.memory_set_val(&mut outputs[0], inventory_id.to_string())?;
    Ok(())
}

pub fn open_inventory_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let request_json: String = plugin.memory_get_val(&inputs[0])?;
    let request: OpenInventoryRequest =
        serde_json::from_str(&request_json).map_err(|e| Error::msg(format!("Invalid open inventory json: {}", e)))?;

    let clients =
        get_clients_container_bridge().ok_or_else(|| Error::msg("ClientsContainer bridge is not initialized"))?;
    let inventory_manager =
        get_inventory_manager_bridge().ok_or_else(|| Error::msg("InventoryManager bridge is not initialized"))?;
    let worlds_manager =
        get_worlds_manager_bridge().ok_or_else(|| Error::msg("WorldsManager bridge is not initialized"))?;
    let items_manager =
        get_items_manager_bridge().ok_or_else(|| Error::msg("ItemsManager bridge is not initialized"))?;

    open_inventory(
        request.get_client_id(),
        request.get_inventory_id(),
        &clients,
        &inventory_manager,
        &worlds_manager,
        &items_manager,
    );

    plugin.memory_set_val(&mut outputs[0], "")?;
    Ok(())
}

pub fn close_inventory_raw(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let request_json: String = plugin.memory_get_val(&inputs[0])?;
    let request: OpenInventoryRequest =
        serde_json::from_str(&request_json).map_err(|e| Error::msg(format!("Invalid close inventory json: {}", e)))?;

    let clients =
        get_clients_container_bridge().ok_or_else(|| Error::msg("ClientsContainer bridge is not initialized"))?;
    let inventory_manager =
        get_inventory_manager_bridge().ok_or_else(|| Error::msg("InventoryManager bridge is not initialized"))?;

    close_inventory(
        request.get_client_id(),
        request.get_inventory_id(),
        &clients,
        &inventory_manager,
    );

    plugin.memory_set_val(&mut outputs[0], "")?;
    Ok(())
}

pub fn register_all<'a>(builder: PluginBuilder<'a>, ctx: &SharedHostContext) -> PluginBuilder<'a> {
    let ctx1 = Arc::clone(ctx);
    let ctx2 = Arc::clone(ctx);

    builder
        .with_function(
            "has_world_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            has_world_raw,
        )
        .with_function(
            "create_world_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            create_world_raw,
        )
        .with_function(
            "register_world_generator_raw",
            [PTR],
            [PTR],
            UserData::new(ctx1),
            register_world_generator_raw,
        )
        .with_function(
            "get_plugin_slug_raw",
            [],
            [PTR],
            UserData::new(ctx2),
            get_plugin_slug_raw,
        )
        .with_function(
            "read_dir_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            read_dir_raw,
        )
        .with_function(
            "read_file_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            read_file_raw,
        )
        .with_function(
            "add_inventory_item_raw",
            [PTR, PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            add_inventory_item_raw,
        )
        .with_function(
            "get_player_world_slug_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            get_player_world_slug_raw,
        )
        .with_function(
            "get_player_inventory_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            get_player_inventory_raw,
        )
        .with_function(
            "add_item_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            add_item_raw,
        )
        .with_function(
            "get_or_create_player_data_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            get_or_create_player_data_raw,
        )
        .with_function(
            "save_player_data_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            save_player_data_raw,
        )
        .with_function(
            "edit_world_block_raw",
            [PTR, PTR, PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            edit_world_block_raw,
        )
        .with_function(
            "get_or_create_inventory_raw",
            [PTR, PTR, PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            get_or_create_inventory_raw,
        )
        .with_function(
            "open_inventory_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            open_inventory_raw,
        )
        .with_function(
            "close_inventory_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            close_inventory_raw,
        )
}
