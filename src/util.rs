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
use crate::constant::KEY_PREFIX;
use crate::crypto::{Address, ArrayLike, Hash};
use anyhow::{Context, Result};
use std::num::ParseIntError;

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

pub fn parse_u64(s: &str) -> Result<u64, ParseIntError> {
    s.parse::<u64>()
}

pub fn key(key_type: &str, param: &str) -> String {
    format!("{}_{}_{}", KEY_PREFIX, key_type, param)
}

pub fn key_without_param(key_type: &str) -> String {
    format!("{}_{}", KEY_PREFIX, key_type)
}
