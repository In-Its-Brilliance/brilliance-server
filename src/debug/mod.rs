pub mod console_commands;
pub mod tps_counter;

use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
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

/// Starts a background thread that periodically checks for deadlocks.
///
/// Uses parking_lot's deadlock detection feature, checking every 1s.
/// When deadlocks are detected, detailed information including backtraces
/// is logged at the error level.
#[cfg(all(debug_assertions, feature = "deadlock_detection"))]
pub fn start_deadlock_detector() {
    use std::thread;
    use std::time::Duration;

    thread::spawn(|| {
        loop {
            thread::sleep(Duration::from_millis(1000));

            let deadlocks = parking_lot::deadlock::check_deadlock();
            if !deadlocks.is_empty() {
                log::error!("{} deadlocks detected!", deadlocks.len());
                for threads in &deadlocks {
                    for t in threads {
                        log::error!("{:?}", t.backtrace());
                    }
                }
            }
        }
    });
    log::info!(target: "debug", "Deadlock detection is enabled!");
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
        app.add_systems(Update, crate::debug::tps_counter::tps_counter_system);
        app.add_systems(Update, crate::debug::tps_counter::tps_broadcast_system.after(crate::debug::tps_counter::tps_counter_system));

        let mut commands_handler = app.world_mut().get_resource_mut::<CommandsHandler>().unwrap();
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_tps(), command_tps));
    }
}
