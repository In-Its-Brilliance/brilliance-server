use common::utils::colors::get_log_level_color;
use log::{Metadata, Record};

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
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            ConsoleHandler::send_message(format_log!(record));
        }
    }

    fn flush(&self) {}
}
