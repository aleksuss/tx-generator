// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use env_logger::{fmt::Formatter, Builder};
use log::{Level, Record, SetLoggerError};
use std::{
    env,
    io::{self, Write},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn init_custom_logger() -> Result<(), SetLoggerError> {
    let mut builder = Builder::new();
    builder.format(format_log_record);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.try_init()
}

fn format_log_record(buf: &mut Formatter, record: &Record) -> io::Result<()> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = ts.as_secs().to_string();
    let millis = (u64::from(ts.subsec_nanos()) / 1_000_000).to_string();

    let verbose_src_path = match env::var("RUST_VERBOSE_PATH") {
        Ok(val) => val.parse::<bool>().unwrap_or(false),
        Err(_) => false,
    };

    let module = record.module_path().unwrap_or("unknown_module");
    let source_path = if verbose_src_path {
        let file = record.file().unwrap_or("unknown_file");
        let line = record.line().unwrap_or(0);
        format!("{}:{}:{}", module, file, line)
    } else {
        module.to_string()
    };

    let level = match record.level() {
        Level::Error => "ERROR",
        Level::Warn => "WARN",
        Level::Info => "INFO",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    };
    writeln!(
        buf,
        "[{} : {:03}] - [ {} ] - {} - {}",
        secs,
        millis,
        level,
        &source_path,
        record.args()
    )
}
