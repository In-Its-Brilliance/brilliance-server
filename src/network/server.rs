use bevy::time::common_conditions::on_timer;
use bevy_app::{App, Update};
use bevy_ecs::change_detection::Mut;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::{
    system::{Res, ResMut},
    world::World,
};
use common::timed_lock;
use common::utils::debug::SmartRwLock;
use common::utils::events::event_channel::{ChannelReader, EventChannel};
use common::utils::events::EventInterface;
use flume::{Receiver, Sender};
use lazy_static::lazy_static;
use network::messages::{ClientMessages, NetworkMessageType, ServerMessages};
use network::server::{ConnectionMessages, IServerConnection, IServerNetwork};
use network::NetworkServer;
use std::sync::Arc;

use super::events::{
    on_connection::{on_connection, PlayerConnectionEvent},
    on_connection_info::{on_connection_info, PlayerConnectionInfoEvent},
    on_disconnect::{on_disconnect, PlayerDisconnectEvent},
    on_edit_block::{on_edit_block, EditBlockEvent},
    on_media_loaded::{on_media_loaded, PlayerMediaLoadedEvent},
    on_player_move::{on_player_move, PlayerMoveEvent},
    on_resources_has_cache::{on_resources_has_cache, ResourcesHasCacheEvent},
    on_settings_loaded::{on_settings_loaded, PlayerSettingsLoadedEvent},
};
use crate::network::chunks_sender::{flush_compressed_chunks, send_chunks, ChunkCompressQueue};
use crate::network::client_network::ClientNetwork;
use crate::network::clients_container::ClientsContainer;
use crate::network::sync_players::PlayerSpawnEvent;
use crate::{console::commands_executer::CommandExecuter, entities::events::on_player_spawn::on_player_spawn};
use crate::{console::commands_executer::CommandsHandler, LaunchSettings};
use crate::{
    entities::entity::{IntoServerPosition, IntoServerRotation},
    network::console_commands::{command_kick, command_parser_kick},
};

const SEND_CHUNKS_DELAY: std::time::Duration = std::time::Duration::from_millis(10);

pub struct NetworkPlugin;

lazy_static! {
    static ref CONSOLE_INPUT: (Sender<(u64, String)>, Receiver<(u64, String)>) = flume::unbounded();
}

/// Holds an `EventChannel<T>` for emitting events (writer/sender side).
///
/// Systems that produce events take `Res<NetworkEventChannel<T>>` and call `.0.emit_event(...)`.
#[derive(Resource)]
pub struct NetworkEventChannel<T: Send + Sync + 'static>(pub EventChannel<T>);

/// Holds a `ChannelReader<T>` for consuming events (reader/receiver side).
///
/// Handler systems take `Res<NetworkEventListener<T>>` and call `.0.iter_events()`.
#[derive(Resource)]
pub struct NetworkEventListener<T: Send + Sync + 'static>(pub ChannelReader<T>);

fn register_network_event<T: Send + Sync + 'static>(app: &mut App) {
    let mut channel = EventChannel::<T>::default();
    let reader = channel.get_reader();
    app.insert_resource(NetworkEventChannel(channel));
    app.insert_resource(NetworkEventListener(reader));
}

#[derive(Resource)]
pub struct NetworkContainer {
    server_network: Arc<SmartRwLock<NetworkServer>>,
}

impl NetworkContainer {
    pub fn new(ip_port: String) -> Self {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let network = rt.block_on(async { NetworkServer::new(ip_port).await });
        let server_network = Arc::new(timed_lock!(network, "server_network"));

        let net_clone = Arc::clone(&server_network);
        std::thread::Builder::new()
            .name("network-step".into())
            .spawn(move || {
                let mut last_instant = std::time::Instant::now();
                loop {
                    let now = std::time::Instant::now();
                    let delta = now - last_instant;
                    last_instant = now;

                    if delta > std::time::Duration::from_millis(100) {
                        log::warn!(target: "network", "Network step thread lag: {:.2?}", delta);
                    }

                    {
                        let net = net_clone.read();
                        rt.block_on(net.step(delta));
                    }

                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            })
            .expect("failed to spawn network-step thread");

        Self { server_network }
    }

    pub fn is_connected(&self, client: &ClientNetwork) -> bool {
        let network = self.server_network.read();
        network.is_connected(client.get_connection())
    }
}

impl NetworkPlugin {
    pub fn build(app: &mut App) {
        let server_settings = app.world().get_resource::<LaunchSettings>().unwrap();
        let ip_port = format!("{}:{}", server_settings.get_args().ip, server_settings.get_args().port);

        log::info!(target: "network", "Starting server on &6{}", ip_port);

        let mut commands_handler = app.world_mut().get_resource_mut::<CommandsHandler>().unwrap();
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_kick(), command_kick));

        app.insert_resource(NetworkContainer::new(ip_port));
        app.insert_resource(ClientsContainer::default());
        app.insert_resource(ChunkCompressQueue::default());

        // Register flume-based event channels
        register_network_event::<ResourcesHasCacheEvent>(app);
        register_network_event::<PlayerConnectionEvent>(app);
        register_network_event::<PlayerConnectionInfoEvent>(app);
        register_network_event::<PlayerDisconnectEvent>(app);
        register_network_event::<PlayerMoveEvent>(app);
        register_network_event::<EditBlockEvent>(app);
        register_network_event::<PlayerMediaLoadedEvent>(app);
        register_network_event::<PlayerSettingsLoadedEvent>(app);

        // Core drain system (replaces receive_message_system + handle_events_system)
        app.add_systems(Update, drain_network_system);

        app.add_systems(
            Update,
            send_chunks
                .after(drain_network_system)
                .run_if(on_timer(SEND_CHUNKS_DELAY)),
        );
        app.add_systems(Update, flush_compressed_chunks.after(drain_network_system));

        app.add_systems(Update, console_client_command_event);

        // Handler systems — all ordered after drain_network_system
        app.add_systems(Update, on_resources_has_cache.after(drain_network_system));
        app.add_systems(Update, on_connection.after(drain_network_system));
        app.add_systems(Update, on_connection_info.after(drain_network_system));
        app.add_systems(Update, on_disconnect.after(drain_network_system));
        app.add_systems(Update, on_player_move.after(drain_network_system));
        app.add_systems(Update, on_edit_block.after(drain_network_system));
        app.add_systems(Update, on_media_loaded.after(drain_network_system));
        app.add_systems(Update, on_settings_loaded.after(drain_network_system));

        // PlayerSpawnEvent stays as Bevy Message (internal ECS event, not network)
        app.add_message::<PlayerSpawnEvent>();
        app.add_systems(Update, on_player_spawn);
    }

    pub(crate) fn send_console_output(client: &ClientNetwork, message: String) {
        let input = ServerMessages::ConsoleOutput { message: message };
        client.send_message(NetworkMessageType::ReliableOrdered, &input);
    }
}

#[cfg(debug_assertions)]
fn span_name_for_client_message(msg: &ClientMessages) -> &'static str {
    match msg {
        ClientMessages::ConnectionInfo { .. } => "server.drain_network_system::ConnectionInfo",
        ClientMessages::ConsoleInput { .. } => "server.drain_network_system::ConsoleInput",
        ClientMessages::PlayerMove { .. } => "server.drain_network_system::PlayerMove",
        ClientMessages::ChunkRecieved { .. } => "server.drain_network_system::ChunkRecieved",
        ClientMessages::EditBlockRequest { .. } => "server.drain_network_system::EditBlockRequest",
        ClientMessages::ResourcesHasCache { .. } => "server.drain_network_system::ResourcesHasCache",
        ClientMessages::ResourcesLoaded { .. } => "server.drain_network_system::ResourcesLoaded",
        ClientMessages::SettingsLoaded => "server.drain_network_system::SettingsLoaded",
    }
}

fn drain_network_system(
    network_container: Res<NetworkContainer>,
    mut clients: ResMut<ClientsContainer>,

    connection_channel: Res<NetworkEventChannel<PlayerConnectionEvent>>,
    disconnect_channel: Res<NetworkEventChannel<PlayerDisconnectEvent>>,
    resources_has_cache_channel: Res<NetworkEventChannel<ResourcesHasCacheEvent>>,
    connection_info_channel: Res<NetworkEventChannel<PlayerConnectionInfoEvent>>,
    player_move_channel: Res<NetworkEventChannel<PlayerMoveEvent>>,
    edit_block_channel: Res<NetworkEventChannel<EditBlockEvent>>,
    player_media_loaded_channel: Res<NetworkEventChannel<PlayerMediaLoadedEvent>>,
    settings_loaded_channel: Res<NetworkEventChannel<PlayerSettingsLoadedEvent>>,
) {
    #[cfg(feature = "trace")]
    let _span = bevy_utils::tracing::info_span!("server.drain_network_system").entered();
    let _s = crate::span!("server.drain_network_system");

    let network = network_container.server_network.read();

    // --- Drain errors ---
    for message in network.drain_errors() {
        log::error!(target: "network", "Network error: {}", message);
    }

    // --- Drain connections/disconnections FIRST ---
    // (so new clients are added before we try to drain their messages)
    {
        let _s = crate::span!("server.drain_network_system::connections");
        for connection in network.drain_connections() {
            match connection {
                ConnectionMessages::Connect { connection } => {
                    clients.add(connection.clone());
                    let client = clients.get(&connection.get_client_id()).unwrap();
                    connection_channel
                        .0
                        .emit_event(PlayerConnectionEvent::new(client.clone()));
                }
                ConnectionMessages::Disconnect { client_id, reason } => {
                    let client = clients.get(&client_id).unwrap();
                    disconnect_channel
                        .0
                        .emit_event(PlayerDisconnectEvent::new(client.clone(), reason));
                }
            }
        }
    }

    // --- Drain client messages ---
    for (client_id, client) in clients.iter() {
        for decoded in client.get_connection().drain_client_messages() {
            #[cfg(debug_assertions)]
            let _s = crate::span!(span_name_for_client_message(&decoded));
            match decoded {
                ClientMessages::ResourcesHasCache { exists } => {
                    let event = ResourcesHasCacheEvent::new(client.clone(), exists);
                    resources_has_cache_channel.0.emit_event(event);
                }
                ClientMessages::ResourcesLoaded { last_index } => {
                    let msg = PlayerMediaLoadedEvent::new(client.clone(), Some(last_index));
                    player_media_loaded_channel.0.emit_event(msg);
                }
                ClientMessages::SettingsLoaded => {
                    let msg = PlayerSettingsLoadedEvent::new(client.clone());
                    settings_loaded_channel.0.emit_event(msg);
                }
                ClientMessages::ConsoleInput { command } => {
                    CONSOLE_INPUT.0.send((*client_id, command)).unwrap();
                }
                ClientMessages::ChunkRecieved { chunk_positions } => {
                    client.mark_chunks_as_recieved(chunk_positions);
                }
                ClientMessages::PlayerMove { position, rotation } => {
                    let movement = PlayerMoveEvent::new(client.clone(), position.to_server(), rotation.to_server());
                    player_move_channel.0.emit_event(movement);
                }
                ClientMessages::ConnectionInfo {
                    login,
                    version,
                    architecture,
                    rendering_device,
                } => {
                    let info =
                        PlayerConnectionInfoEvent::new(client.clone(), login, version, architecture, rendering_device);
                    connection_info_channel.0.emit_event(info);
                }
                ClientMessages::EditBlockRequest {
                    world_slug,
                    position,
                    new_block_info,
                } => {
                    let edit = EditBlockEvent::new(client.clone(), world_slug, position, new_block_info);
                    edit_block_channel.0.emit_event(edit);
                }
            }
        }
    }
}

#[allow(unused_mut)]
fn console_client_command_event(world: &mut World) {
    let _s = crate::span!("server.console_client_command_event");
    world.resource_scope(|world, mut clients: Mut<ClientsContainer>| {
        for (client_id, command) in CONSOLE_INPUT.1.try_iter() {
            let client = clients.get(&client_id).unwrap();
            CommandsHandler::execute_command(world, Box::new(client.clone()), &command);
        }
    });
}
