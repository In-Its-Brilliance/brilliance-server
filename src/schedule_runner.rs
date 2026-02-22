use bevy_app::{App, AppExit, Plugin, PluginsState};
use std::time::{Duration, Instant};

/// Spin-wait threshold: the last portion of the tick interval
/// that uses spin-waiting instead of OS sleep for precise timing.
const SPIN_THRESHOLD: Duration = Duration::from_millis(7);

/// Custom schedule runner plugin that uses hybrid sleep+spin-wait
/// for precise tick timing on Linux where `std::thread::sleep()`
/// can overshoot by 2-5ms due to OS timer granularity.
pub struct PreciseScheduleRunnerPlugin {
    tick_duration: Duration,
}

impl PreciseScheduleRunnerPlugin {
    pub fn new(tps: f64) -> Self {
        Self {
            tick_duration: Duration::from_secs_f64(1.0 / tps),
        }
    }
}

impl Plugin for PreciseScheduleRunnerPlugin {
    fn build(&self, app: &mut App) {
        let tick_duration = self.tick_duration;

        app.set_runner(move |mut app: App| -> AppExit {
            // Complete plugin loading (same as bevy's ScheduleRunnerPlugin)
            let plugins_state = app.plugins_state();
            if plugins_state != PluginsState::Cleaned {
                while app.plugins_state() == PluginsState::Adding {
                    std::hint::spin_loop();
                }
                app.finish();
                app.cleanup();
            }

            loop {
                let tick_start = Instant::now();

                app.update();

                if let Some(exit) = app.should_exit() {
                    return exit;
                }

                let execution_time = tick_start.elapsed();
                if execution_time >= tick_duration {
                    continue;
                }

                let remaining = tick_duration - execution_time;

                // Hybrid sleep: OS sleep for the bulk, spin-wait for precision
                if remaining > SPIN_THRESHOLD {
                    std::thread::sleep(remaining - SPIN_THRESHOLD);
                }

                // Spin-wait until exact deadline
                let deadline = tick_start + tick_duration;
                while Instant::now() < deadline {
                    std::hint::spin_loop();
                }
            }
        });
    }
}
