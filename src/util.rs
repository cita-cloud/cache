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
use crate::constant::{COMMITTED_TX, HASH_TO_RECEIPT, HASH_TO_TX, KEY_PREFIX, UNCOMMITTED_TX};
use crate::crypto::{Address, ArrayLike, Crypto, Hash};
use anyhow::{anyhow, Context, Result};
use std::num::ParseIntError;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[allow(dead_code)]
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

pub fn key(key_type: &str, param: &str) -> String {
    format!("{}_{}_{}", KEY_PREFIX, key_type, param)
}

pub fn key_without_param(key_type: &str) -> String {
    format!("{}_{}", KEY_PREFIX, key_type)
}

pub fn uncommitted_tx_key() -> String {
    format!("{}_{}", KEY_PREFIX, UNCOMMITTED_TX)
}

pub fn committed_tx_key() -> String {
    format!("{}_{}", KEY_PREFIX, COMMITTED_TX)
}

pub fn hash_to_tx() -> String {
    format!("{}_{}", KEY_PREFIX, HASH_TO_TX)
}

pub fn hash_to_receipt() -> String {
    format!("{}_{}", KEY_PREFIX, HASH_TO_RECEIPT)
}

pub fn timestamp() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs() as u64 * 1000u64
        + (since_the_epoch.subsec_nanos() as f64 / 1_000_000.0) as u64
}
