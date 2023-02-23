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

use crate::{CacheConfig, ControllerClient, CryptoClient, EvmClient, ExecutorClient, RpcClients};
use efficient_sm2::KeyPair;
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
pub const STREAM_TYPE: &str = "stream";

pub const ROLLUP_WRITE_ENABLE: &str = "rollup_write_enable";

pub const CITA_CLOUD_BLOCK_NUMBER: &str = "cita_cloud_block_number";
pub const CURRENT_BATCH_NUMBER: &str = "current_batch_number";
pub const VALIDATOR_BATCH_NUMBER: &str = "validator_batch_number";
pub const CURRENT_FAKE_BLOCK_HASH: &str = "current_fake_block_hash";
pub const PACKAGED_TX: &str = "packaged_tx";
pub const STREAM_ID: &str = "stream_id";
pub const ENQUEUE: &str = "enqueue";
pub const EXPIRE: &str = "expire";

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

pub static CACHE_CONFIG: OnceCell<CacheConfig> = OnceCell::const_new();
pub static RPC_CLIENTS: OnceCell<
    RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
> = OnceCell::const_new();
pub static KEY_PAIR: OnceCell<KeyPair> = OnceCell::const_new();

pub fn config() -> CacheConfig {
    CACHE_CONFIG.get().unwrap().clone()
}

fn rpc_clients() -> RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient> {
    RPC_CLIENTS.get().unwrap().clone()
}

pub fn rough_internal() -> u64 {
    config().rough_internal.unwrap_or_default() * ONE_THOUSAND
}

pub fn block_count() -> u64 {
    config().packaged_tx_vub.unwrap_or_default()
}

// #[warn(dead_code)]
// pub fn crypto() -> CryptoClient {
//     rpc_clients().crypto
// }

pub fn controller() -> ControllerClient {
    rpc_clients().controller
}

pub fn local_executor() -> ExecutorClient {
    rpc_clients().local_executor
}

pub fn evm() -> EvmClient {
    rpc_clients().evm
}

// pub fn keypair() -> KeyPair {
//     KEY_PAIR.get().unwrap().clone()
// }
