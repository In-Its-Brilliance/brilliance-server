use bevy::{prelude::TaskPoolPlugin, time::TimePlugin};
use bevy_app::{App, ScheduleRunnerPlugin};
use common::utils::print_logo;
use debug::DebugPlugin;
use launch_settings::{get_log_level, LaunchSettings};
use plugins::PluginApp;
use std::time::Duration;
use worlds::WorldsHandlerPlugin;

use tracing_subscriber::layer::SubscriberExt;

use crate::console::ConsolePlugin;
use crate::{
    logger::{TracingToLogLayer, CONSOLE_LOGGER},
    network::{runtime_plugin::RuntimePlugin, server::NetworkPlugin},
};

mod console;
mod debug;
mod entities;
pub mod launch_settings;
mod logger;
mod network;
mod plugins;
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

    // Bridge tracing events (from extism) to the log crate
    let tracing_level = match log_level {
        log::LevelFilter::Off => tracing_subscriber::filter::LevelFilter::OFF,
        log::LevelFilter::Error => tracing_subscriber::filter::LevelFilter::ERROR,
        log::LevelFilter::Warn => tracing_subscriber::filter::LevelFilter::WARN,
        log::LevelFilter::Info => tracing_subscriber::filter::LevelFilter::INFO,
        log::LevelFilter::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
        log::LevelFilter::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
    };
    let subscriber = tracing_subscriber::registry()
        .with(TracingToLogLayer)
        .with(tracing_level);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    print_logo(VERSION);
    log::debug!(target: "main", "Log level using: {}", log_level);
    log::info!(target: "main", "In Its Brilliance Server version &d{}", VERSION);

    let mut app = App::new();
    app.insert_resource(server_settings);
    app.add_plugins((
        TimePlugin::default(),
        TaskPoolPlugin::default(),
        ScheduleRunnerPlugin::default(),
        RuntimePlugin::default(),
        PluginApp::default(),
        ConsolePlugin::default(),
        WorldsHandlerPlugin::default(),
        DebugPlugin::default(),
    ));
    NetworkPlugin::build(&mut app);
    app.run();
}
