pub mod console_commands;
pub mod tps_counter;

use bevy_app::{App, FixedUpdate, Plugin, Startup};
#[cfg(debug_assertions)]
use common::utils::debug::runtime_storage::RuntimeStorage;

use console_commands::{command_parser_tps, command_tps};
#[cfg(debug_assertions)]
use lazy_static::lazy_static;

use crate::console::commands_executer::{CommandExecuter, CommandsHandler};

#[cfg(debug_assertions)]
pub mod runtime_profiler;

#[cfg(debug_assertions)]
pub mod runtime_reporter;

#[macro_export]
macro_rules! span {
    ($name:expr) => {{
        #[cfg(debug_assertions)]
        {
            Some($crate::debug::runtime_profiler::RuntimeSpan::new($name))
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }};
}

#[cfg(debug_assertions)]
lazy_static! {
    pub static ref STORAGE: std::sync::Mutex<RuntimeStorage> = std::sync::Mutex::new(RuntimeStorage::new());
}

pub struct DebugPlugin;

impl Default for DebugPlugin {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, crate::debug::tps_counter::tps_counter_init);
        app.add_systems(FixedUpdate, crate::debug::tps_counter::tps_counter_system);

        let mut commands_handler = app.world_mut().get_resource_mut::<CommandsHandler>().unwrap();
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_tps(), command_tps));
    }
}
