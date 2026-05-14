use extism::*;
use common::{
    chunks::{
        block_position::BlockPosition,
        chunk_data::BlockDataInfo,
    },
};
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use serde_json;
use serde::Deserialize;
use crate::{
    network::{client_network::WorldEntity, clients_container::ClientsContainer},
    network::sync_world_change::sync_world_block_change,
    worlds::worlds_manager::WorldsManager,
};

pub type SharedHostContext = Arc<parking_lot::Mutex<HostContext>>;

static WORLDS_MANAGER_PTR: AtomicPtr<WorldsManager> = AtomicPtr::new(std::ptr::null_mut());
static CLIENTS_CONTAINER_PTR: AtomicPtr<ClientsContainer> = AtomicPtr::new(std::ptr::null_mut());

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

pub fn set_worlds_manager_bridge(worlds_manager: &WorldsManager) {
    WORLDS_MANAGER_PTR.store(worlds_manager as *const WorldsManager as *mut WorldsManager, Ordering::SeqCst);
}

pub fn set_clients_container_bridge(clients_container: &ClientsContainer) {
    CLIENTS_CONTAINER_PTR.store(
        clients_container as *const ClientsContainer as *mut ClientsContainer,
        Ordering::SeqCst,
    );
}

fn get_worlds_manager_bridge() -> Option<&'static WorldsManager> {
    let ptr = WORLDS_MANAGER_PTR.load(Ordering::SeqCst);
    unsafe { ptr.as_ref() }
}

fn get_clients_container_bridge() -> Option<&'static ClientsContainer> {
    let ptr = CLIENTS_CONTAINER_PTR.load(Ordering::SeqCst);
    unsafe { ptr.as_ref() }
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
    let worlds_manager = get_worlds_manager_bridge().ok_or_else(|| Error::msg("WorldsManager bridge is not initialized"))?;
    let has_world = worlds_manager.has_world_with_slug(&slug);
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
    let clients = get_clients_container_bridge().ok_or_else(|| Error::msg("ClientsContainer bridge is not initialized"))?;
    let world_slug = clients
        .get(&client_id)
        .and_then(|client| client.get_world_entity())
        .map(|world_entity: WorldEntity| world_entity.get_world_slug().clone())
        .unwrap_or_default();
    plugin.memory_set_val(&mut outputs[0], world_slug)?;
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

    let position_json: BlockPositionJson = serde_json::from_str(&position_json)
        .map_err(|e| Error::msg(format!("Invalid block position json: {}", e)))?;
    let position = BlockPosition::new(
        position_json.x.floor() as i64,
        position_json.y.floor() as i64,
        position_json.z.floor() as i64,
    );
    let new_block_info: Option<BlockDataInfo> = serde_json::from_str(&new_block_info_json)
        .map_err(|e| Error::msg(format!("Invalid block data json: {}", e)))?;

    let worlds_manager = get_worlds_manager_bridge()
        .ok_or_else(|| Error::msg("WorldsManager bridge is not initialized"))?;
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
            "edit_world_block_raw",
            [PTR, PTR, PTR],
            [PTR],
            UserData::new(Arc::clone(ctx)),
            edit_world_block_raw,
        )
}
