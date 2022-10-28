use crate::current_time;
use log::{Level, Metadata, Record};
use rocket::yansi::Paint;

pub static LOGGER: CacheLogger = CacheLogger;

pub struct CacheLogger;

impl log::Log for CacheLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        match log::max_level().to_level() {
            Some(max) => metadata.level() <= max,
            None => false,
        }
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = record.level();
            match level {
                Level::Error => println!(
                    "[{}] [{}] {}",
                    Paint::red(record.level()).wrap(),
                    current_time(),
                    Paint::red(record.args()).wrap()
                ),
                Level::Warn => println!(
                    "[{}] [{}] {}",
                    Paint::yellow(record.level()).wrap(),
                    current_time(),
                    Paint::yellow(record.args()).wrap()
                ),
                Level::Info => println!(
                    "[{}] [{}] {}",
                    Paint::green(record.level()).wrap(),
                    current_time(),
                    Paint::green(record.args()).wrap()
                ),
                Level::Debug => println!(
                    "[{}] [{}] {}",
                    Paint::green(record.level()).wrap(),
                    current_time(),
                    Paint::green(record.args()).wrap()
                ),
                Level::Trace => println!(
                    "[{}] [{}] {}",
                    Paint::black(record.level()).wrap(),
                    current_time(),
                    Paint::black(record.args()).wrap()
                ),
            }
        }
    }
    fn flush(&self) {}
}
