use common::utils::debug::format_grouped_lines::format_grouped_lines;
use common::utils::debug::runtime_storage::SpansType;
use std::time::Duration;

use crate::network::runtime_plugin::RuntimePlugin;

const SPIKE_THRESHOLD: Duration = Duration::from_millis(10);

macro_rules! spike_template {
    () => {
        "&cTick spike! ({tps:.1} tps) Total: {duration:.1?}&r
{lines}"
    };
}

pub struct RuntimeReporter;

impl RuntimeReporter {
    /// Проверяет, превысил ли текущий тик порог SPIKE_THRESHOLD (сумма "last" root-спанов).
    pub fn check_spike(spans: &SpansType, tps: &f32) {
        if RuntimePlugin::is_stopped() {
            return;
        }

        if spans.is_empty() {
            return;
        }

        let total_root: Duration = spans
            .iter()
            .filter(|(name, _)| !name.contains("::"))
            .map(|(_, (_, _, last))| *last)
            .sum();

        if total_root < SPIKE_THRESHOLD {
            return;
        }

        let mut items: Vec<(&'static str, Duration, Duration)> = spans
            .iter()
            .map(|(name, (total, count, last))| {
                let avg = if *count > 0 { *total / *count } else { Duration::ZERO };
                (*name, *last, avg)
            })
            .collect();

        items.sort_by(|a, b| b.1.cmp(&a.1));

        let (lines, duration) = format_grouped_lines(items);
        let msg = format!(
            spike_template!(),
            tps = tps,
            duration = duration,
            lines = lines,
        );

        log::warn!(target: "debug", "{}", msg);
    }
}
