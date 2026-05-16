use crate::{
    clients::{client::WorldEntity, clients_container::ClientsContainer},
    inventory::{
        commands::{close_inventory, get_or_create_inventory, open_inventory},
        inventory_manager::InventoryManager,
    },
    network::sync_world_change::sync_world_block_change,
    storage::storage_manager::StorageManager,
    worlds::worlds_manager::WorldsManager,
};
use common::{
    chunks::{
        block_position::BlockPosition,
        chunk_data::BlockDataInfo,
    },
    plugin_api::inventory::OpenInventoryRequest,
    server_storage::taits::{IServerStorage, PlayerData},
    utils::debug::SmartRwLock,
};
use extism::*;
use rand::random;
use serde::Deserialize;
use serde_json;
use std::sync::{Arc, OnceLock};

pub type SharedHostContext = Arc<parking_lot::Mutex<HostContext>>;

static WORLDS_MANAGER_BRIDGE: OnceLock<Arc<SmartRwLock<WorldsManager>>> = OnceLock::new();
static SERVER_STORAGE_BRIDGE: OnceLock<Arc<SmartRwLock<StorageManager>>> = OnceLock::new();
static CLIENTS_CONTAINER_BRIDGE: OnceLock<Arc<SmartRwLock<ClientsContainer>>> = OnceLock::new();
static INVENTORY_MANAGER_BRIDGE: OnceLock<Arc<SmartRwLock<InventoryManager>>> = OnceLock::new();

#[derive(Default)]
pub struct HostContext {
    plugin_slug: String,
    world_generators: Vec<String>,
    has_on_chunk_generate: bool,
}

impl HostContext {
    pub fn create(plugin_slug: String) -> Self {
        Self {
            plugin_slug,
            ..Default::default()
        }
    }

    pub fn get_plugin_slug(&self) -> &String {
        &self.plugin_slug
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

pub fn create_inventory_raw(
    plugin: &mut CurrentPlugin,
    _inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<SharedHostContext>,
) -> Result<(), Error> {
    let inventory_id = random::<u64>();
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

    open_inventory(
        request.get_client_id(),
        request.get_inventory_id(),
        &clients,
        &inventory_manager,
        &worlds_manager,
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
            "get_player_world_slug_raw",
            [PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            get_player_world_slug_raw,
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
            "create_inventory_raw",
            [],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            create_inventory_raw,
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
