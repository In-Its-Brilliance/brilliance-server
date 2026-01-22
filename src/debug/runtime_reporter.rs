use common::utils::debug::format_grouped_lines::format_grouped_lines;
use common::utils::debug::runtime_storage::SpansType;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const REPORT_COOLDOWN: Duration = Duration::from_secs(10);
pub(crate) const TPS_THRESHOLD: f32 = 55.0;

macro_rules! lags_template {
    () => {
        "&cLags detected! ({tps:.1} tps) Total: {duration:.1}ms:&r
{lines}"
    };
}

pub struct RuntimeReporter;

static LAST_REPORT: Mutex<Option<Instant>> = Mutex::new(None);

impl RuntimeReporter {
    pub fn report(spans: &SpansType, tps: &f32) -> bool {
        if spans.is_empty() {
            return false;
        }

        if *tps >= TPS_THRESHOLD {
            return false;
        }

        {
            let mut last = LAST_REPORT.lock().unwrap();
            if let Some(t) = *last {
                if t.elapsed() < REPORT_COOLDOWN {
                    return false;
                }
            }
            *last = Some(Instant::now());
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
            lags_template!(),
            tps = tps,
            duration = duration.as_secs_f64() * 1000.0,
            lines = lines,
        );

        log::warn!(target: "debug", "{}", msg);
        true
    }
}
