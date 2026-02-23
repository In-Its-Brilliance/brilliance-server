use bevy::prelude::*;
use network::messages::{NetworkMessageType, ServerMessages};
use std::time::Duration;

use crate::launch_settings::LaunchSettings;
use crate::network::clients_container::ClientsContainer;
use crate::network::server::NetworkContainer;

#[derive(Resource)]
pub struct TpsCounter {
    ticks: u32,
    timer: Timer,
    tps: f32,
    tps_updated: bool,
}

impl TpsCounter {
    pub fn get_tps(&self) -> &f32 {
        &self.tps
    }
}

pub(crate) fn tps_counter_init(mut commands: Commands) {
    commands.insert_resource(TpsCounter {
        ticks: 0,
        timer: Timer::new(Duration::from_secs(1), TimerMode::Repeating),
        tps: 0.0,
        tps_updated: false,
    });
}

pub(crate) fn tps_counter_system(time: Res<Time>, mut counter: ResMut<TpsCounter>) {
    let _s = crate::span!("debug.tps_counter_system");
    counter.ticks += 1;
    counter.timer.tick(time.delta());

    if counter.timer.is_finished() {
        counter.tps = counter.ticks as f32;
        counter.ticks = 0;
        counter.tps_updated = true;
    }

    #[cfg(debug_assertions)]
    {
        if counter.tps == 0.0 {
            return;
        }

        if let Ok(storage) = crate::debug::STORAGE.try_lock() {
            crate::debug::runtime_reporter::RuntimeReporter::check_spike(storage.get_spans(), counter.get_tps());
        }
    }
}

/// Broadcasts server TPS to all connected clients (once per second, when TPS updates).
pub(crate) fn tps_broadcast_system(
    mut counter: ResMut<TpsCounter>,
    settings: Res<LaunchSettings>,
    clients: Res<ClientsContainer>,
    network_container: Res<NetworkContainer>,
) {
    if !counter.tps_updated {
        return;
    }
    counter.tps_updated = false;

    if !settings.get_args().send_tps {
        return;
    }

    let msg = ServerMessages::ServerStatus { tps: counter.tps };
    for (_client_id, client) in clients.iter() {
        if !network_container.is_connected(client) {
            continue;
        }
        client.send_message(NetworkMessageType::Unreliable, &msg);
    }
}
