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
use crate::common::crypto::{Address, ArrayLike, Crypto, Hash};
use anyhow::{anyhow, Context, Result};
use crossbeam::atomic::AtomicCell;
use std::num::ParseIntError;
use std::time::{SystemTime, UNIX_EPOCH};
use time::UtcOffset;

static LOCAL_UTC_OFFSET: AtomicCell<Option<UtcOffset>> = AtomicCell::new(None);

/// This should be called without any other concurrent running threads.
pub fn init_local_utc_offset() {
    let local_utc_offset =
        UtcOffset::current_local_offset().unwrap_or_else(|_| UtcOffset::from_hms(8, 0, 0).unwrap());

    LOCAL_UTC_OFFSET.store(Some(local_utc_offset));
}

/// Call init_utc_offset first without any other concurrent running threads. Otherwise UTC+8 is used.
/// This is due to a potential race condition.
/// [CVE-2020-26235](https://github.com/chronotope/chrono/issues/602)
pub fn display_time(timestamp: u64) -> String {
    let local_offset = LOCAL_UTC_OFFSET
        .load()
        .unwrap_or_else(|| UtcOffset::from_hms(8, 0, 0).unwrap());
    let format = time::format_description::parse(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]",
    )
        .unwrap();
    time::OffsetDateTime::from_unix_timestamp((timestamp / 1000) as i64)
        .expect("invalid timestamp")
        .to_offset(local_offset)
        .format(&format)
        .unwrap()
}

pub fn current_time() -> String {
    display_time(timestamp())
}

pub fn remove_0x(s: &str) -> &str {
    s.strip_prefix("0x").unwrap_or(s)
}

pub fn parse_data(s: &str) -> Result<Vec<u8>> {
    hex::decode(remove_0x(s)).context("invalid hex input")
}

pub fn parse_addr(s: &str) -> Result<Address> {
    let input = parse_data(s)?;
    Address::try_from_slice(&input)
}

pub fn parse_hash(s: &str) -> Result<Hash> {
    let input = parse_data(s)?;
    Hash::try_from_slice(&input)
}

pub fn parse_value(s: &str) -> Result<[u8; 32]> {
    let s = remove_0x(s);
    if s.len() > 64 {
        return Err(anyhow!("can't parse value, the given str is too long"));
    }
    // padding 0 to 32 bytes
    let padded = format!("{:0>64}", s);
    hex::decode(&padded)
        .map(|v| v.try_into().unwrap())
        .map_err(|e| anyhow!("invalid value: {e}"))
}

pub fn parse_u64(s: &str) -> Result<u64, ParseIntError> {
    s.parse::<u64>()
}

pub fn hex(data: &[u8]) -> String {
    format!("0x{}", hex::encode(data))
}

pub fn hex_without_0x(data: &[u8]) -> String {
    hex::encode(data)
}

#[allow(dead_code)]
pub fn parse_pk<C: Crypto>(s: &str) -> Result<C::PublicKey> {
    let input = parse_data(s)?;
    C::PublicKey::try_from_slice(&input)
}

#[allow(dead_code)]
pub fn parse_sk<C: Crypto>(s: &str) -> Result<C::SecretKey> {
    let input = parse_data(s)?;
    C::SecretKey::try_from_slice(&input)
}

pub fn timestamp() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs() as u64 * 1000u64
        + (since_the_epoch.subsec_nanos() as f64 / 1_000_000.0) as u64
}
