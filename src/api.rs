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

use std::future::Future;
use crate::constant::{FAILURE, SUCCESS, SUCCESS_MESSAGE};
use crate::display::Display;
use anyhow::{Error, Result};
use rocket::serde::Serialize;
use utoipa::Component;
use utoipa::OpenApi;

use crate::util::{key, key_without_param, parse_addr, parse_hash, parse_u64, remove_0x};
use cita_cloud_proto::{
    blockchain::{
        raw_transaction::Tx, Block, BlockHeader, RawTransaction, RawTransactions, Transaction,
        UnverifiedTransaction, Witness,
    },
    common::{NodeInfo, NodeNetInfo, TotalNodeInfo},
    controller::{SystemConfig, Flag},
    evm::{Balance, ByteAbi, ByteCode, Log, Nonce, Receipt},
};
use r2d2_redis::redis::ConnectionLike;
use r2d2_redis::redis::{ToRedisArgs, FromRedisValue};
use rocket::{Request, State};

use rocket::request::{FromRequest, Outcome};
use rocket::serde::json::Json;
use serde_json::Value;
use crate::redis::{Pool};
use r2d2_redis::redis::Commands;
use rocket::http::Status;
use r2d2::{Error as R2d2Err, PooledConnection};
use r2d2_redis::redis::RedisError as OperateErr;
use r2d2_redis::RedisConnectionManager;
use rocket::futures::TryFutureExt;
use crate::{ControllerClient, EvmClient, ExecutorClient};
use crate::context::Context;
use crate::core::controller::ControllerBehaviour;
use crate::core::evm::EvmBehaviour;
use crate::crypto::Address;
use crate::error::ValidateError;
use crate::from_request::{Entity, ParamType, ValidateResult};


#[catch(404)]
pub fn uri_not_found() -> Json<String> {
    Json(String::from("URI not found"))
}

#[catch(404)]
pub fn api_not_found() -> Json<String> {
    Json(String::from("api not found"))
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
pub struct QueryResult<T> {
    pub status: u64,
    pub data: Option<T>,
    pub message: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
#[component(example = json ! ({"status": 1, "message": "success"}))]
pub struct SuccessResult {
    pub status: u64,
    pub message: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
#[component(example = json ! ({"status": 0, "message": "error message"}))]
pub struct FailureResult {
    pub status: u64,
    pub message: String,
}


fn match_result_new<T, E: std::fmt::Display>(result: ValidateResult<T, E>) -> Json<QueryResult<T>> {
    match result {
        ValidateResult::Ok(num) => Json(QueryResult {
            status: SUCCESS,
            data: Some(num),
            message: SUCCESS_MESSAGE.to_string(),
        }),
        ValidateResult::Err(e) => Json(QueryResult {
            status: FAILURE,
            data: None,
            message: format!("{}", e),
        }),
    }
}

///Get current block number
#[get("/get-block-number")]
#[utoipa::path(get, path = "/api/get-block-number")]
pub async fn block_number(result: ValidateResult<Entity<u64>, ValidateError>,
                          ctx: Context<ControllerClient, ExecutorClient, EvmClient>) -> Json<QueryResult<u64>> {
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };

    if entity.val == 0 {
        match ctx.controller.get_block_number(false).await {
            Ok(val) => {
                ctx.get_redis_connection().set::<String, u64, String>(entity.key, val);
                success(val)
            }
            Err(e) => fail(e),
        }
    } else {
        success(entity.val)
    }
}

fn success<T>(val: T) -> Json<QueryResult<T>> {
    Json(QueryResult {
        status: SUCCESS,
        data: Some(val),
        message: SUCCESS_MESSAGE.to_string(),
    })
}

fn fail<T, E: std::fmt::Display>(e: E) -> Json<QueryResult<T>> {
    Json(QueryResult {
        status: FAILURE,
        data: None,
        message: format!("{}", e),
    })
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
    result: ValidateResult<Entity<String>, ValidateError>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<String>> {
    println!("get-abi address {}", address);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if let ParamType::Address(address) = entity.param {
        if entity.val == String::default() {
            match ctx.evm.get_abi(address).await {
                Ok(val) => {
                    let val_str = val.display();
                    ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                    success(val_str)
                }
                Err(e) => fail(e),
            }
        } else {
            success(entity.val)
        }
    } else {
        success(String::default())
    }
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
    result: ValidateResult<Entity<String>, ValidateError>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<String>> {
    println!("get-balance address {}", address);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match entity.param {
            ParamType::Address(address) => match ctx.evm.get_balance(address).await {
                Ok(val) => {
                    let val_str = val.display();
                    println!("get-balance value {}", val_str);

                    ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                    success(val_str)
                }
                Err(e) => fail(e),
            },
            _ => fail("param type not match!"),
        }
    } else {
        success(entity.val)
    }
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
    result: ValidateResult<Entity<String>, ValidateError>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<String>> {
    println!("get-block hash_or_height {}", hash_or_height);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match entity.param {
            ParamType::NumOrHash(height, hash) => if height == 0 {
                match ctx.controller.get_block_detail_by_hash(hash).await {
                    Ok(val) => {
                        let val_str = val.display();

                        ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                        success(val_str)
                    }
                    Err(e) => fail(e),
                }
            } else {
                match ctx.controller.get_block_detail_by_number(height).await {
                    Ok(val) => {
                        let val_str = val.display();

                        ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                        success(val_str)
                    }
                    Err(e) => fail(e),
                }
            },
            _ => fail("para type not match!"),
        }
    } else {
        success(entity.val)
    }
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
    result: ValidateResult<Entity<String>, ValidateError>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<String>> {
    println!("get-code address {}", address);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match entity.param {
            ParamType::Address(address) => match ctx.evm.get_code(address).await {
                Ok(val) => {
                    let val_str = val.display();
                    println!("get-code value {}", val_str);

                    ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                    success(val_str)
                }
                Err(e) => fail(e),
            },
            _ => fail("param type not match!")
        }
    } else {
        success(entity.val)
    }
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
pub async fn tx(hash: &str, result: ValidateResult<Entity<u64>, ValidateError>,
          ctx: Context<ControllerClient, ExecutorClient, EvmClient>, ) -> Json<QueryResult<u64>> {
    println!("get-tx hash {}", hash);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == 0 {
        match ctx.controller.get_peer_count().await {
            Ok(val) => {
                println!("get-peers-count value {}", val);

                ctx.get_redis_connection().set::<String, u64, String>(entity.key, val);
                success(val)
            }
            Err(e) => fail(e),
        }
    } else {
        success(entity.val)
    }
}

///Get peers count
#[get("/get-peers-count")]
#[utoipa::path(get, path = "/api/get-peers-count")]
pub async fn peers_count(result: ValidateResult<Entity<u64>, ValidateError>,
                         ctx: Context<ControllerClient, ExecutorClient, EvmClient>, ) -> Json<QueryResult<u64>> {
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == 0 {
        match ctx.controller.get_peer_count().await {
            Ok(val) => {
                ctx.get_redis_connection().set::<String, u64, String>(entity.key, val);
                success(val)
            }
            Err(e) => fail(e),
        }
    } else {
        success(entity.val)
    }
}

///Get peers info
#[get("/get-peers-info")]
#[utoipa::path(get, path = "/api/get-peers-info")]
pub async fn peers_info(result: ValidateResult<Entity<String>, ValidateError>,
                  ctx: Context<ControllerClient, ExecutorClient, EvmClient>, ) -> Json<QueryResult<String>> {
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match ctx.controller.get_peers_info().await {
            Ok(val) => {
                let val_str = val.display();
                println!("get-peers-info value {}", val_str);

                ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                success(val_str.clone())
            }
            Err(e) => fail(e),
        }
    } else {
        success(entity.val)
    }
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
    result: ValidateResult<Entity<String>, ValidateError>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<String>> {
    println!("get-account-nonce address {}", address);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match entity.param {
            ParamType::Address(address) => match ctx.evm.get_tx_count(address).await {
                Ok(val) => {
                    let val_str = val.display();
                    println!("get-account-nonce value {}", val_str);

                    ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                    success(val_str)
                }
                Err(e) => fail(e),
            },
            _ => fail("param type not match!")
        }
    } else {
        success(entity.val)
    }
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
pub async fn receipt(hash: &str, result: ValidateResult<Entity<String>, ValidateError>,
               ctx: Context<ControllerClient, ExecutorClient, EvmClient>, ) -> Json<QueryResult<String>> {
    println!("get-receipt hash {}", hash);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match entity.param {
            ParamType::Hash(hash) => match ctx.evm.get_receipt(hash).await {
                Ok(val) => {
                    let val_str = val.display();
                    println!("get-receipt hash {}", val_str);

                    ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                    success(val_str)
                }
                Err(e) => fail(e),
            },
            _ => fail("param type not match!")
        }
    } else {
        success(entity.val)
    }
}

///Get chain version
#[get("/get-version")]
#[utoipa::path(get, path = "/api/get-version")]
pub async fn version(result: ValidateResult<Entity<String>, ValidateError>,
               ctx: Context<ControllerClient, ExecutorClient, EvmClient>, ) -> Json<QueryResult<String>> {
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match ctx.controller.get_version().await {
            Ok(val) => {
                let val = val.to_string();
                ctx.get_redis_connection().set::<String, String, String>(entity.key, val.clone());
                success(val)
            }
            Err(e) => fail(e),
        }
    } else {
        success(entity.val)
    }
}

///Get system config
#[get("/get-system-config")]
#[utoipa::path(get, path = "/api/get-system-config")]
pub async fn system_config(result: ValidateResult<Entity<String>, ValidateError>,
                     ctx: Context<ControllerClient, ExecutorClient, EvmClient>,) -> Json<QueryResult<String>> {
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match ctx.controller.get_system_config().await {
            Ok(val) => {
                let val_str = val.display();
                ctx.get_redis_connection().set::<String, String, String>(entity.key, val_str.clone());
                success(val_str)
            }
            Err(e) => fail(e),
        }
    } else {
        success(entity.val)
    }
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
    result: ValidateResult<Entity<String>, ValidateError>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<String>> {
    println!("get-block-hash block_number {}", block_number);
    let entity = match result {
        ValidateResult::Ok(entity) => entity,
        ValidateResult::Err(e) => return fail(e),
    };
    if entity.val == String::default() {
        match entity.param {
            ParamType::Number(height) => match ctx.controller.get_block_hash(height).await {
                Ok(val) => {
                    let val_str = val.display();
                    ctx.get_redis_connection().set::< String, String, String > (entity.key, val_str.clone());
                    success(val_str)
                }
                Err(e) => fail(e),
            },
            _ => fail("param type not match!")

        }
    } else {
        success(entity.val)
    }
}

#[derive(OpenApi)]
#[openapi(
handlers(
block_number,
abi,
balance,
block,
code,
tx,
peers_count,
peers_info,
account_nonce,
receipt,
system_config,
block_hash,
version
),
components(SuccessResult, FailureResult)
)]
pub struct ApiDoc;
