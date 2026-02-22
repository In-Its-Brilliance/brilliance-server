use bevy::prelude::*;
use std::time::Duration;

const TRACE_FLUSH_EVERY_N_TICKS: u32 = 30;

#[derive(Resource)]
pub struct TpsCounter {
    ticks: u32,
    timer: Timer,
    tps: f32,

    #[cfg(debug_assertions)]
    trace_flush_counter: u32,
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

        #[cfg(debug_assertions)]
        trace_flush_counter: 0,
    });
}

pub(crate) fn tps_counter_system(time: Res<Time>, mut counter: ResMut<TpsCounter>) {
    let _s = crate::span!("debug.tps_counter_system");
    counter.ticks += 1;
    counter.timer.tick(time.delta());

    if counter.timer.is_finished() {
        counter.tps = counter.ticks as f32;
        counter.ticks = 0;
    }

    #[cfg(debug_assertions)]
    {
        // Репортим только после первого измерения TPS (через 1 секунду после старта)
        if counter.tps == 0.0 {
            return;
        }

        counter.trace_flush_counter = counter.trace_flush_counter.wrapping_add(1);

        if counter.trace_flush_counter % TRACE_FLUSH_EVERY_N_TICKS == 0 {
            if let Ok(mut storage) = crate::debug::STORAGE.try_lock() {
                let clear =
                    crate::debug::runtime_reporter::RuntimeReporter::report(storage.get_spans(), counter.get_tps());
                if clear {
                    storage.clear();
                }
            }
        }
    }
}
