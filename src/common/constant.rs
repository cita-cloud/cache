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

use crate::redis::Pool;
use crate::{ControllerClient, CryptoClient, EvmClient, ExecutorClient};
use tokio::sync::OnceCell;

pub const SUCCESS: u64 = 1;
pub const FAILURE: u64 = 0;
pub const ONE_THOUSAND: u64 = 1000;
pub const SUCCESS_MESSAGE: &str = "success";
pub const KEY_PREFIX: &str = "cache";
pub const HASH_TYPE: &str = "hash";
#[allow(dead_code)]
pub const SET_TYPE: &str = "set";
pub const ZSET_TYPE: &str = "zset";
pub const VAL_TYPE: &str = "val";

pub const ROLLUP_WRITE_ENABLE: &str = "rollup_write_enable";

pub const CITA_CLOUD_BLOCK_NUMBER: &str = "cita_cloud_block_number";
pub const CURRENT_BATCH_NUMBER: &str = "current_batch_number";
pub const VALIDATOR_BATCH_NUMBER: &str = "validator_batch_number";
pub const CURRENT_FAKE_BLOCK_HASH: &str = "current_fake_block_hash";

pub const VALIDATE_TX_BUFFER: &str = "validate_tx_hash";

pub const SYSTEM_CONFIG: &str = "system_config";
pub const ADMIN_ACCOUNT: &str = "admin_account";
pub const UNCOMMITTED_TX: &str = "uncommitted_tx_hash";
pub const PACK_UNCOMMITTED_TX: &str = "pack_uncommitted_tx_hash";
pub const COMMITTED_TX: &str = "committed_tx_hash";
pub const HASH_TO_TX: &str = "hash_to_tx";
pub const HASH_TO_BLOCK_NUMBER: &str = "hash_to_block_number";
pub const TIME_TO_CLEAN_UP: &str = "time_to_clean_up";
pub const LAZY_EVICT_TO_TIME: &str = "lazy_evict_to_time";
pub const EVICT_TO_ROUGH_TIME: &str = "evict_to_rough_time";
pub const RECEIPT: &str = "receipt";
pub const TX: &str = "tx";
pub const CONTRACT_KEY: &str = "contract";
pub const EXPIRED_KEY_EVENT_AT_ALL_DB: &str = "__keyevent@*__:expired";

pub static REDIS_POOL: OnceCell<Pool> = OnceCell::const_new();
pub static CONTROLLER_CLIENT: OnceCell<ControllerClient> = OnceCell::const_new();
pub static EXECUTOR_CLIENT: OnceCell<ExecutorClient> = OnceCell::const_new();
pub static LOCAL_EXECUTOR_CLIENT: OnceCell<ExecutorClient> = OnceCell::const_new();
pub static EVM_CLIENT: OnceCell<EvmClient> = OnceCell::const_new();
pub static LOCAL_EVM_CLIENT: OnceCell<EvmClient> = OnceCell::const_new();
pub static CRYPTO_CLIENT: OnceCell<CryptoClient> = OnceCell::const_new();
pub static ROUGH_INTERNAL: OnceCell<u64> = OnceCell::const_new();

pub fn rough_internal() -> u64 {
    *ROUGH_INTERNAL.get().unwrap() * ONE_THOUSAND
}

pub fn controller() -> ControllerClient {
    CONTROLLER_CLIENT.get().unwrap().clone()
}

pub fn local_executor() -> ExecutorClient {
    LOCAL_EXECUTOR_CLIENT.get().unwrap().clone()
}

pub fn evm() -> EvmClient {
    EVM_CLIENT.get().unwrap().clone()
}
