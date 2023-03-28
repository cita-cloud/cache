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
use crate::common::crypto::sm::{sm2_generate_secret_key, sm2_sign};
use crate::common::display::Display;
use crate::common::util::{parse_addr, parse_hash, parse_u64, remove_0x};
use crate::core::key_manager::{key, CacheOnly};
use crate::core::rpc_clients::RpcClients;
use crate::redis::Pool;
use crate::rest_api::common::{failure, success, CacheResult};
use crate::{
    BlockContext, CacheBehavior, CacheConfig, ControllerClient, CryptoClient, EvmClient,
    ExecutorClient, Hash,
};
use anyhow::anyhow;
use rocket::serde::json::Json;
use rocket::State;
use serde_json::{json, Value};
use tracing::instrument;

///Get version
#[get("/get-version/<flag>")]
#[utoipa::path(get, path = "/api/get-version/{flag}",
params(
("flag", description = "The flag"),
))]
#[instrument(skip_all)]
pub async fn version(flag: bool) -> Json<CacheResult<Value>> {
    if flag {
        // let keypair = keypair();
        //
        // keypair
        //     .sign(Hash::default().as_slice())
        //     .expect("sm2 sign failed");

        sm2_sign(Hash::default().as_slice(), &sm2_generate_secret_key());
    }
    Json(success(json!(1)))
}

///Get current block number
#[get("/get-block-number")]
#[utoipa::path(get, path = "/api/get-block-number")]
#[instrument(skip_all)]
pub async fn block_number(pool: &State<Pool>) -> Json<CacheResult<Value>> {
    let con = &mut pool.get();
    match BlockContext::current_cita_height(con) {
        Ok(height) => Json(success(json!(height))),
        Err(e) => Json(failure(anyhow!(e))),
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
#[instrument(skip_all)]
pub async fn abi(
    address: &str,
    config: &State<CacheConfig>,
    pool: &State<Pool>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-abi address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    let con = &mut pool.get();

    match CacheOnly::load_or_query_proto(
        con,
        key("abi".to_string(), address.to_string()),
        config.expire_time.unwrap(),
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
#[instrument(skip_all)]
pub async fn balance(
    address: &str,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-balance address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    let con = &mut pool.get();
    match CacheOnly::load_or_query_proto(
        con,
        key("balance".to_string(), address.to_string()),
        config.expire_time.unwrap(),
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
#[instrument(skip_all)]
pub async fn block(
    hash_or_height: &str,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-block hash_or_height {}", hash_or_height);
    let hash_or_height = remove_0x(hash_or_height);
    let expire_time = config.expire_time.unwrap();
    let con = &mut pool.get();
    let result = if let Ok(data) = parse_u64(hash_or_height) {
        CacheOnly::load_or_query_proto(
            con,
            key("block".to_string(), hash_or_height.to_string()),
            expire_time,
            ctx.controller.get_block_by_number(data),
        )
        .await
    } else {
        match parse_hash(hash_or_height) {
            Ok(data) => {
                CacheOnly::load_or_query_proto(
                    con,
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
#[instrument(skip_all)]
pub async fn code(
    address: &str,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-code address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    let con = &mut pool.get();
    match CacheOnly::load_or_query_proto(
        con,
        key("code".to_string(), address.to_string()),
        config.expire_time.unwrap(),
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
#[instrument(skip_all)]
pub async fn tx(
    hash: &str,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-tx hash {}", hash);
    let hash = remove_0x(hash);
    let data = match parse_hash(hash) {
        Ok(hash) => hash,
        Err(e) => return Json(failure(e)),
    };
    let con = &mut pool.get();
    match CacheOnly::load_or_query_proto(
        con,
        key("tx".to_string(), hash.to_string()),
        config.expire_time.unwrap(),
        ctx.controller.get_tx(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get nonce by account address
#[get("/get-account-nonce/<address>")]
#[utoipa::path(
get,
path = "/api/get-account-nonce/{address}",
params(
("address", description = "The account address"),
)
)]
#[instrument(skip_all)]
pub async fn account_nonce(
    address: &str,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-account-nonce address {}", address);
    let address = remove_0x(address);
    let data = match parse_addr(address) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    let con = &mut pool.get();
    match CacheOnly::load_or_query_proto(
        con,
        key("account-nonce".to_string(), address.to_string()),
        config.expire_time.unwrap(),
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
#[instrument(skip_all)]
pub async fn receipt(
    hash: &str,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-receipt hash {}", hash);
    let hash = remove_0x(hash);
    let data = match parse_hash(hash) {
        Ok(hash) => hash,
        Err(e) => return Json(failure(e)),
    };
    let con = &mut pool.get();
    match CacheOnly::load_or_query_proto(
        con,
        key("receipt".to_string(), hash.to_string()),
        config.expire_time.unwrap(),
        ctx.evm.get_receipt(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get local tx receipt by hash
#[get("/get-receipt-local/<hash>")]
#[utoipa::path(
get,
path = "/api/get-receipt-local/{hash}",
params(
("hash", description = "The tx hash"),
)
)]
#[instrument(skip_all)]
pub async fn receipt_local(
    hash: &str,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-receipt inner hash {}", hash);
    let hash = remove_0x(hash);
    let data = match parse_hash(hash) {
        Ok(hash) => hash,
        Err(e) => return Json(failure(e)),
    };
    let con = &mut pool.get();
    match CacheOnly::load_or_query_proto(
        con,
        key("receipt-inner".to_string(), hash.to_string()),
        config.expire_time.unwrap(),
        ctx.local_evm.get_receipt(data),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}

///Get system config
#[get("/get-system-config")]
#[utoipa::path(get, path = "/api/get-system-config")]
#[instrument(skip_all)]
pub async fn system_config(pool: &State<Pool>) -> Json<CacheResult<Value>> {
    let con = &mut pool.get();
    match BlockContext::system_config(con) {
        Ok(config) => Json(success(json!(config.to_json()))),
        Err(e) => Json(failure(anyhow!(e))),
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
#[instrument(skip_all)]
pub async fn block_hash(
    block_number: usize,
    pool: &State<Pool>,
    config: &State<CacheConfig>,
    ctx: &State<RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    info!("get-block-hash block_number {}", block_number);
    let con = &mut pool.get();
    match CacheOnly::load_or_query_array_like(
        con,
        key("block-hash".to_string(), block_number.to_string()),
        config.expire_time.unwrap(),
        ctx.controller.get_block_hash(block_number as u64),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}
