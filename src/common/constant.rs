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

pub const SUCCESS: u64 = 1;
pub const FAILURE: u64 = 0;
pub const ONE_MIN: u64 = 60_000;
pub const SUCCESS_MESSAGE: &str = "success";
pub const KEY_PREFIX: &str = "cache";
pub const HASH_TYPE: &str = "hash";
#[allow(dead_code)]
pub const SET_TYPE: &str = "set";
pub const ZSET_TYPE: &str = "zset";
pub const VAL_TYPE: &str = "val";

pub const UNCOMMITTED_TX: &str = "uncommitted_tx_hash";
pub const COMMITTED_TX: &str = "committed_tx_hash";
pub const HASH_TO_TX: &str = "hash_to_tx";
pub const HASH_TO_BLOCK_NUMBER: &str = "hash_to_block_number";
pub const TIME_TO_CLEAN_UP: &str = "time_to_clean_up";
pub const EVICT_KEY_TO_TIME: &str = "evict_key_to_time";
pub const RECEIPT: &str = "receipt";
pub const TX: &str = "tx";
pub const CONTRACT_KEY: &str = "contract";
