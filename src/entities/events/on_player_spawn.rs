use bevy::prelude::Res;
use bevy_ecs::message::MessageReader;
use common::plugin_api::events::player_spawn::PlayerSpawnEvent as PluginPlayerSpawnEvent;

use crate::{
    clients::client::Client,
    entities::skin::EntitySkinComponent,
    network::{
        sync_entities::sync_entity_spawn,
        sync_players::{send_entities_for_player, PlayerSpawnEvent},
    },
    plugins::plugins_manager::PluginsManager,
    worlds::worlds_manager::SharedWorldsManager,
};

/// Спавн игрока в мире
///
/// Вызывается при успешном подключении игрока если чанк прогружен
/// или при прогрузке чанка, если entity попал в загружающийся чанк.
pub(crate) fn on_player_spawn(
    worlds_manager: Res<SharedWorldsManager>,
    plugins_manager: Res<PluginsManager>,
    mut connection_events: MessageReader<PlayerSpawnEvent>,
) {
    #[cfg(feature = "trace")]
    let _span = bevy_utils::tracing::info_span!("on_player_spawn.on_player_spawn").entered();
    let _s = crate::span!("on_player_spawn.on_player_spawn");

    for event in connection_events.read() {
        let target_entity = event.world_entity.get_entity();
        let worlds_manager_guard = worlds_manager.read();
        let world_manager = worlds_manager_guard
            .get_world_manager(event.world_entity.get_world_slug())
            .unwrap();

        send_entities_for_player(&world_manager, target_entity);

        let ecs = world_manager.get_ecs();
        let entity_ref = ecs.get_entity(target_entity).unwrap();

        // Sync his entity if exists
        if entity_ref.get::<EntitySkinComponent>().is_some() {
            sync_entity_spawn(&*world_manager, target_entity);
        }

        let plugin_event =
            PluginPlayerSpawnEvent::create(entity_ref.get::<Client>().unwrap().get_client_id());
        plugins_manager.dispatch_player_spawn_event(&plugin_event);
    }
}
