// Copyright Rivtower Technologies LLC.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::common::util::current_time;
use crate::config;
use anyhow::Result;
use log::{set_logger, set_max_level, Level, LevelFilter, Metadata, Record};
use rocket::yansi::Paint;
use std::thread;

pub static LOGGER: CacheLogger = CacheLogger;

pub struct CacheLogger;

impl CacheLogger {
    pub fn set_up() -> Result<()> {
        let config = config();
        set_logger(&LOGGER)?;
        set_max_level(LevelFilter::from(config.log_level));
        Ok(())
    }
}

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
                    "[{}] [{}] [{:?}] {}",
                    Paint::red(record.level()).wrap(),
                    current_time(),
                    Paint::cyan(thread::current().id()),
                    Paint::red(record.args()).wrap()
                ),
                Level::Warn => {
                    println!(
                        "[{}] [{}] [{:?}] {}",
                        Paint::yellow(record.level()).wrap(),
                        current_time(),
                        Paint::cyan(thread::current().id()),
                        Paint::yellow(record.args()).wrap()
                    )
                }
                Level::Info => println!(
                    "[{}] [{}] [{:?}] {}",
                    Paint::green(record.level()).wrap(),
                    current_time(),
                    Paint::cyan(thread::current().id()),
                    Paint::green(record.args()).wrap()
                ),
                Level::Debug => println!(
                    "[{}] [{}] [{:?}] {}",
                    Paint::green(record.level()).wrap(),
                    current_time(),
                    Paint::cyan(thread::current().id()),
                    Paint::green(record.args()).wrap()
                ),
                Level::Trace => println!(
                    "[{}] [{}] [{:?}] {}",
                    Paint::black(record.level()).wrap(),
                    current_time(),
                    Paint::cyan(thread::current().id()),
                    Paint::black(record.args()).wrap()
                ),
            }
        }
    }
    fn flush(&self) {}
}
