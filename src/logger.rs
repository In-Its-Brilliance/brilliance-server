use std::fmt::Write;

use common::utils::colors::get_log_level_color;
use log::{Metadata, Record};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;

use crate::console::console_handler::ConsoleHandler;
pub(crate) static CONSOLE_LOGGER: ConsoleLogger = ConsoleLogger;
pub(crate) struct ConsoleLogger;

#[cfg(debug_assertions)]
macro_rules! format_log {
    ($record:expr) => {
        format!(
            "&8{target:<9} {level_color}{level}&r: {msg}",
            target = $record.metadata().target(),
            level_color = get_log_level_color(&$record.level()),
            level = $record.level(),
            msg = $record.args()
        )
    };
}

#[cfg(not(debug_assertions))]
macro_rules! format_log {
    ($record:expr) => {
        format!(
            "{level_color}{level}&r: {msg}",
            level_color = get_log_level_color(&$record.level()),
            level = $record.level(),
            msg = $record.args()
        )
    };
}

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if metadata.target() == "rustyline" {
            return false;
        }
        #[cfg(not(debug_assertions))]
        if metadata.target().starts_with("renetcode") {
            return false;
        }
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            ConsoleHandler::send_message(format_log!(record));
        }
    }

    fn flush(&self) {}
}

/// Bridges tracing events (from extism) to the `log` crate,
/// so they go through ConsoleLogger with the server's log format.
pub(crate) struct TracingToLogLayer;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for TracingToLogLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let target = meta.target();

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let level = match *meta.level() {
            tracing::Level::ERROR => log::Level::Error,
            tracing::Level::WARN => log::Level::Warn,
            tracing::Level::INFO => log::Level::Info,
            tracing::Level::DEBUG => log::Level::Debug,
            tracing::Level::TRACE => log::Level::Trace,
        };

        log::log!(target: target, level, "{}", visitor.message);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{:?}", value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        }
    }
}
