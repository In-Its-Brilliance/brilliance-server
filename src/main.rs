use crate::console::ConsolePlugin;
use crate::{
    logger::CONSOLE_LOGGER,
    network::{runtime_plugin::RuntimePlugin, server::NetworkPlugin},
};
use bevy::{prelude::TaskPoolPlugin, time::TimePlugin};
use bevy_app::{App, ScheduleRunnerPlugin};
use client_resources::ResourcesPlugin;
use launch_settings::{get_log_level, LaunchSettings};
use log::info;
use std::time::Duration;
use worlds::WorldsHandlerPlugin;

mod client_resources;
mod console;
mod entities;
pub mod launch_settings;
mod logger;
mod network;
mod worlds;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const CHUNKS_DISTANCE: u16 = 12;
pub const CHUNKS_DESPAWN_TIMER: Duration = Duration::from_secs(5);
pub static SEND_CHUNK_QUEUE_LIMIT: usize = 64;

fn main() {
    log::set_logger(&CONSOLE_LOGGER).unwrap();

    let server_settings = LaunchSettings::new();

    let log_level = get_log_level(&server_settings.get_args().logs);
    log::set_max_level(log_level.clone());
    info!(target: "main", "Log level using: {}", log_level);

    info!(target: "main", "Brilliance Server version &d{}", VERSION);

    let mut app = App::new();
    app.insert_resource(server_settings);
    app.add_plugins((
        TimePlugin::default(),
        TaskPoolPlugin::default(),
        ScheduleRunnerPlugin::default(),
        RuntimePlugin::default(),
        ConsolePlugin::default(),
        ResourcesPlugin::default(),
        WorldsHandlerPlugin::default(),
    ));
    NetworkPlugin::build(&mut app);
    app.run();
}
