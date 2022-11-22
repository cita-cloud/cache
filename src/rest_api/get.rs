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

extern crate rocket;

use crate::cita_cloud::controller::ControllerBehaviour;
use crate::cita_cloud::evm::EvmBehaviour;
use crate::common::display::Display;
use crate::common::util::{parse_addr, parse_hash, parse_u64, remove_0x};
use crate::core::context::Context;
use crate::core::key_manager::{key, CacheBehavior, CacheManager};
use crate::rest_api::common::{failure, success, CacheResult};
use crate::{CacheConfig, ControllerClient, CryptoClient, EvmClient, ExecutorClient};
use rocket::serde::json::Json;
use rocket::State;
use serde_json::{json, Value};

///Get current block number
#[get("/get-block-number")]
#[utoipa::path(get, path = "/api/get-block-number")]
pub async fn block_number(
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    match ctx.controller.get_block_number(false).await {
        Ok(block_number) => Json(success(json!(block_number))),
        Err(e) => Json(failure(e)),
    }
}

///Get contract abi by contract address
#[get("/get-abi/<address>")]
#[utoipa::path(
get,
path = "/api/get-abi/{address}",
params(
("address", description = "The contract address"),
)
)]
pub async fn abi(
    address: &str,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-abi address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    match CacheManager::load_or_query(
        key("abi".to_string(), address.to_string()),
        config.expire_time.unwrap_or_default() as usize,
        ctx.evm.get_abi(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get balance by account address
#[get("/get-balance/<address>")]
#[utoipa::path(
get,
path = "/api/get-balance/{address}",
params(
("address", description = "The account address"),
)
)]
pub async fn balance(
    address: &str,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-balance address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    match CacheManager::load_or_query(
        key("balance".to_string(), address.to_string()),
        config.expire_time.unwrap_or_default() as usize,
        ctx.evm.get_balance(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get block by height or hash
#[get("/get-block/<hash_or_height>")]
#[utoipa::path(
get,
path = "/api/get-block/{hash_or_height}",
params(
("hash_or_height", description = "The block hash or height"),
)
)]
pub async fn block(
    hash_or_height: &str,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-block hash_or_height {}", hash_or_height);
    let hash_or_height = remove_0x(hash_or_height);
    let expire_time = config.expire_time.unwrap_or_default() as usize;
    let result = if let Ok(data) = parse_u64(hash_or_height) {
        CacheManager::load_or_query(
            key("block".to_string(), hash_or_height.to_string()),
            expire_time,
            ctx.controller.get_block_by_number(data),
        )
        .await
    } else {
        match parse_hash(hash_or_height) {
            Ok(data) => {
                CacheManager::load_or_query(
                    key("block".to_string(), hash_or_height.to_string()),
                    expire_time,
                    ctx.controller.get_block_by_hash(data),
                )
                .await
            }
            Err(e) => Err(e),
        }
    };
    match result {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get code by contract address
#[get("/get-code/<address>")]
#[utoipa::path(
get,
path = "/api/get-code/{address}",
params(
("address", description = "The contract address"),
)
)]
pub async fn code(
    address: &str,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-code address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    match CacheManager::load_or_query(
        key("code".to_string(), address.to_string()),
        config.expire_time.unwrap_or_default() as usize,
        ctx.evm.get_code(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get tx by hash
#[get("/get-tx/<hash>")]
#[utoipa::path(
get,
path = "/api/get-tx/{hash}",
params(
("hash", description = "The tx hash"),
)
)]
pub async fn tx(
    hash: &str,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-tx hash {}", hash);
    let hash = remove_0x(hash);
    let data = match parse_hash(hash) {
        Ok(hash) => hash,
        Err(e) => return Json(failure(e)),
    };
    match CacheManager::load_or_query(
        key("tx".to_string(), hash.to_string()),
        config.expire_time.unwrap_or_default() as usize,
        ctx.controller.get_tx(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

// ///Get peers count
// #[get("/get-peers-count")]
// #[utoipa::path(get, path = "/api/get-peers-count")]
// pub async fn peers_count(
//     ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
// ) -> Json<CacheResult<Value>> {
//     match ctx.controller.get_peer_count().await {
//         Ok(peer_count) => Json(success(json!(peer_count))),
//         Err(e) => Json(failure(e)),
//     }
// }

// ///Get peers info
// #[get("/get-peers-info")]
// #[utoipa::path(get, path = "/api/get-peers-info")]
// pub async fn peers_info(
//     ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
// ) -> Json<CacheResult<Value>> {
//     match ctx.controller.get_peers_info().await {
//         Ok(peers_info) => Json(success(peers_info.to_json())),
//         Err(e) => Json(failure(e)),
//     }
// }

///Get nonce by account address
#[get("/get-account-nonce/<address>")]
#[utoipa::path(
get,
path = "/api/get-account-nonce/{address}",
params(
("address", description = "The account address"),
)
)]
pub async fn account_nonce(
    address: &str,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-account-nonce address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    match CacheManager::load_or_query(
        key("account-nonce".to_string(), address.to_string()),
        config.expire_time.unwrap_or_default() as usize,
        ctx.evm.get_tx_count(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get tx receipt by hash
#[get("/get-receipt/<hash>")]
#[utoipa::path(
get,
path = "/api/get-receipt/{hash}",
params(
("hash", description = "The tx hash"),
)
)]
pub async fn receipt(
    hash: &str,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-receipt hash {}", hash);
    let hash = remove_0x(hash);
    let data = match parse_hash(hash) {
        Ok(hash) => hash,
        Err(e) => return Json(failure(e)),
    };
    match CacheManager::load_or_query(
        key("receipt".to_string(), hash.to_string()),
        config.expire_time.unwrap_or_default() as usize,
        ctx.evm.get_receipt(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

// ///Get chain version
// #[get("/get-version")]
// #[utoipa::path(get, path = "/api/get-version")]
// pub async fn version(
//     ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
// ) -> Json<CacheResult<Value>> {
//     match ctx.controller.get_version().await {
//         Ok(version) => Json(success(json!(version))),
//         Err(e) => Json(failure(e)),
//     }
// }

///Get system config
#[get("/get-system-config")]
#[utoipa::path(get, path = "/api/get-system-config")]
pub async fn system_config(
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    match ctx.controller.get_system_config().await {
        Ok(system_config) => Json(success(system_config.to_json())),
        Err(e) => Json(failure(e)),
    }
}

///Get block hash by block number
#[get("/get-block-hash/<block_number>")]
#[utoipa::path(
get,
path = "/api/get-block-hash/{block_number}",
params(
("block_number", description = "The block number"),
)
)]
pub async fn block_hash(
    block_number: usize,
    config: &State<CacheConfig>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-block-hash block_number {}", block_number);
    match CacheManager::load_or_query(
        key("block-hash".to_string(), block_number.to_string()),
        config.expire_time.unwrap_or_default() as usize,
        ctx.controller.get_block_hash(block_number as u64),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}
