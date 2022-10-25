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

use crate::context::Context;
use crate::rest_api::common::CacheResult;
use crate::{ControllerClient, CryptoClient, EvmClient, ExecutorClient};
use rocket::serde::json::Json;
use serde_json::Value;

///Get current block number
#[get("/get-block-number")]
#[utoipa::path(get, path = "/api/get-block-number")]
pub async fn block_number(
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    Json(result)
}

///Get contract abi by contract address
#[get("/get-abi/<address>")]
#[utoipa::path(
get,
path = "/api/get-abi/{address}",
params(
("address" = String, path, description = "The contract address"),
)
)]
pub async fn abi(
    address: &str,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-abi address {}", address);
    Json(result)
}

///Get balance by account address
#[get("/get-balance/<address>")]
#[utoipa::path(
get,
path = "/api/get-balance/{address}",
params(
("address" = String, path, description = "The account address"),
)
)]
pub async fn balance(
    address: &str,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-balance address {}", address);
    Json(result)
}

///Get block by height or hash
#[get("/get-block/<hash_or_height>")]
#[utoipa::path(
get,
path = "/api/get-block/{hash_or_height}",
params(
("hash_or_height" = String, path, description = "The block hash or height"),
)
)]
pub async fn block(
    hash_or_height: &str,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-block hash_or_height {}", hash_or_height);
    Json(result)
}

///Get code by contract address
#[get("/get-code/<address>")]
#[utoipa::path(
get,
path = "/api/get-code/{address}",
params(
("address" = String, path, description = "The contract address"),
)
)]
pub async fn code(
    address: &str,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-code address {}", address);
    Json(result)
}

///Get tx by hash
#[get("/get-tx/<hash>")]
#[utoipa::path(
get,
path = "/api/get-tx/{hash}",
params(
("hash" = String, path, description = "The tx hash"),
)
)]
pub async fn tx(
    hash: &str,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-tx hash {}", hash);
    Json(result)
}

///Get peers count
#[get("/get-peers-count")]
#[utoipa::path(get, path = "/api/get-peers-count")]
pub async fn peers_count(
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    Json(result)
}

///Get peers info
#[get("/get-peers-info")]
#[utoipa::path(get, path = "/api/get-peers-info")]
pub async fn peers_info(
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    Json(result)
}

///Get nonce by account address
#[get("/get-account-nonce/<address>")]
#[utoipa::path(
get,
path = "/api/get-account-nonce/{address}",
params(
("address" = String, path, description = "The account address"),
)
)]
pub async fn account_nonce(
    address: &str,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-account-nonce address {}", address);
    Json(result)
}

///Get tx receipt by hash
#[get("/get-receipt/<hash>")]
#[utoipa::path(
get,
path = "/api/get-receipt/{hash}",
params(
("hash" = String, path, description = "The tx hash"),
)
)]
pub async fn receipt(
    hash: &str,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-receipt hash {}", hash);
    Json(result)
}

///Get chain version
#[get("/get-version")]
#[utoipa::path(get, path = "/api/get-version")]
pub async fn version(
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    Json(result)
}

///Get system config
#[get("/get-system-config")]
#[utoipa::path(get, path = "/api/get-system-config")]
pub async fn system_config(
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    Json(result)
}

///Get block hash by block number
#[get("/get-block-hash/<block_number>")]
#[utoipa::path(
get,
path = "/api/get-block-hash/{block_number}",
params(
("block_number" = usize, path, description = "The block number"),
)
)]
pub async fn block_hash(
    block_number: usize,
    result: CacheResult<Value>,
    _ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    println!("get-block-hash block_number {}", block_number);
    Json(result)
}
