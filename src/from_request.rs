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

use crate::util::{hkey, key, key_without_param, parse_addr, parse_hash, parse_u64, remove_0x};
use rocket::{Request, State};

use crate::context::Context;
use crate::core::account::MaybeLocked;
use crate::core::controller::ControllerBehaviour;
use crate::core::evm::EvmBehaviour;
use crate::display::Display;
use crate::error::CacheError;
use crate::redis::{hget, load, set};
use crate::{ControllerClient, EvmClient, ExecutorClient};
use rocket::http::Method::Get;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use serde_json::{json, Value};
use std::result::Result;
use std::string::String;

fn get_param<'r>(req: &'r Request<'_>, index: usize) -> &'r str {
    if let Some(val) = req.param(index) {
        val.ok().unwrap()
    } else {
        ""
    }
}

#[derive(Debug)]
pub enum CacheResult<T, E> {
    Ok(T),
    Err(E),
}

fn path_count(req: &Request) -> usize {
    req.uri().path().split('/').count()
}

fn is_obj(pattern: &str) -> bool {
    matches!(
        pattern,
        "peers-info" | "system-config" | "tx" | "block" | "receipt"
    )
}

fn with_param(req: &Request) -> bool {
    path_count(req) == vec!["", "api", "{query-name}", "{param}"].len()
}

fn query_error(path: &str, detail: anyhow::Error) -> CacheError {
    CacheError::QueryCitaCloud {
        query_type: path.replace('-', " "),
        detail,
    }
}

async fn save_and_get(
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient>>,
    path: &str,
    param: &str,
    key: String,
) -> Result<Value, CacheError> {
    match path {
        "block-number" => match ctx.controller.get_block_number(false).await {
            Ok(block_number) => match set(ctx.get_redis_connection(), key, block_number) {
                Ok(_) => Ok(json!(block_number)),
                Err(e) => Err(CacheError::Operate(e)),
            },
            Err(detail) => Err(query_error(path, detail)),
        },
        "peers-count" => match ctx.controller.get_peer_count().await {
            Ok(peer_count) => match set(ctx.get_redis_connection(), key, peer_count) {
                Ok(_) => Ok(json!(peer_count)),
                Err(e) => Err(CacheError::Operate(e)),
            },
            Err(detail) => Err(query_error(path, detail)),
        },
        "version" => match ctx.controller.get_version().await {
            Ok(version) => match set(ctx.get_redis_connection(), key, version.clone()) {
                Ok(_) => Ok(json!(version)),
                Err(e) => Err(CacheError::Operate(e)),
            },
            Err(detail) => Err(query_error(path, detail)),
        },
        "peers-info" => match ctx.controller.get_peers_info().await {
            Ok(info) => match set(ctx.get_redis_connection(), key, info.display()) {
                Ok(_) => Ok(info.to_json()),
                Err(e) => Err(CacheError::Operate(e)),
            },
            Err(detail) => Err(query_error(path, detail)),
        },
        "system-config" => match ctx.controller.get_system_config().await {
            Ok(config) => match set(ctx.get_redis_connection(), key, config.display()) {
                Ok(_) => Ok(config.to_json()),
                Err(e) => Err(CacheError::Operate(e)),
            },
            Err(detail) => Err(query_error(path, detail)),
        },
        "abi" => match parse_addr(param) {
            Ok(data) => match ctx.evm.get_abi(data).await {
                Ok(abi) => match set(ctx.get_redis_connection(), key, abi.display()) {
                    Ok(_) => Ok(json!(abi.display())),
                    Err(e) => Err(CacheError::Operate(e)),
                },
                Err(detail) => Err(query_error(path, detail)),
            },
            Err(e) => Err(CacheError::ParseAddress(e)),
        },
        "account-nonce" => match parse_addr(param) {
            Ok(data) => match ctx.evm.get_tx_count(data).await {
                Ok(nonce) => match set(ctx.get_redis_connection(), key, nonce.display()) {
                    Ok(_) => Ok(json!(nonce.display())),
                    Err(e) => Err(CacheError::Operate(e)),
                },
                Err(detail) => Err(query_error(path, detail)),
            },
            Err(e) => Err(CacheError::ParseAddress(e)),
        },
        "balance" => match parse_addr(param) {
            Ok(data) => match ctx.evm.get_balance(data).await {
                Ok(balance) => match set(ctx.get_redis_connection(), key, balance.display()) {
                    Ok(_) => Ok(json!(balance.display())),
                    Err(e) => Err(CacheError::Operate(e)),
                },
                Err(detail) => Err(query_error(path, detail)),
            },
            Err(e) => Err(CacheError::ParseAddress(e)),
        },
        "code" => match parse_addr(param) {
            Ok(data) => match ctx.evm.get_code(data).await {
                Ok(code) => match set(ctx.get_redis_connection(), key, code.display()) {
                    Ok(_) => Ok(json!(code.display())),
                    Err(e) => Err(CacheError::Operate(e)),
                },
                Err(detail) => Err(query_error(path, detail)),
            },
            Err(e) => Err(CacheError::ParseAddress(e)),
        },
        "block-hash" => match parse_u64(param) {
            Ok(data) => match ctx.controller.get_block_hash(data).await {
                Ok(hash) => match set(ctx.get_redis_connection(), key, hash.display()) {
                    Ok(_) => Ok(json!(hash.display())),
                    Err(e) => Err(CacheError::Operate(e)),
                },
                Err(detail) => Err(query_error(path, detail)),
            },
            Err(e) => Err(CacheError::ParseInt(e)),
        },
        "receipt" => {
            if let Ok(data) = parse_hash(param) {
                match ctx.evm.get_receipt(data).await {
                    Ok(receipt) => match set(ctx.get_redis_connection(), key, receipt.display()) {
                        Ok(_) => Ok(receipt.to_json()),
                        Err(e) => Err(CacheError::Operate(e)),
                    },
                    Err(detail) => Err(query_error(path, detail)),
                }
            } else {
                Err(CacheError::ParseHash)
            }
        }
        "tx" => {
            if let Ok(data) = parse_hash(param) {
                match ctx.controller.get_tx(data).await {
                    Ok(tx) => match set(ctx.get_redis_connection(), key, tx.display()) {
                        Ok(_) => Ok(tx.to_json()),
                        Err(e) => Err(CacheError::Operate(e)),
                    },
                    Err(detail) => Err(query_error(path, detail)),
                }
            } else {
                Err(CacheError::ParseHash)
            }
        }
        "block" => {
            if let Ok(data) = parse_u64(param) {
                match ctx.controller.get_block_by_number(data).await {
                    Ok(block) => match set(ctx.get_redis_connection(), key, block.display()) {
                        Ok(_) => Ok(block.to_json()),
                        Err(e) => Err(CacheError::Operate(e)),
                    },
                    Err(detail) => Err(query_error(path, detail)),
                }
            } else if let Ok(data) = parse_hash(param) {
                match ctx.controller.get_block_by_hash(data).await {
                    Ok(block) => match set(ctx.get_redis_connection(), key, block.display()) {
                        Ok(_) => Ok(block.to_json()),
                        Err(e) => Err(CacheError::Operate(e)),
                    },
                    Err(detail) => Err(query_error(path, detail)),
                }
            } else {
                Err(CacheError::ParseHash)
            }
        }
        _ => Err(CacheError::Uri),
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Context<ControllerClient, ExecutorClient, EvmClient> {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let ctx = req
            .guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient>>>()
            .await
            .unwrap();
        Outcome::Success(Context {
            controller: ctx.controller.clone(),
            executor: ctx.executor.clone(),
            evm: ctx.evm.clone(),
            redis_pool: ctx.redis_pool.clone(),
        })
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CacheResult<Value, CacheError> {
    type Error = CacheError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        if req.method() != Get {
            return Outcome::Forward(());
        }
        let uri = req.uri();
        if !uri.is_normalized() {
            return Outcome::Failure((Status::NotFound, CacheError::Uri));
        }

        let ctx = req
            .guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient>>>()
            .await
            .unwrap();
        let pattern = &get_param(req, 0)["get-".len()..];
        let param = remove_0x(get_param(req, 1));

        let key = if !with_param(req) {
            return match save_and_get(ctx, pattern, param, key_without_param(pattern)).await {
                Ok(data) => Outcome::Success(CacheResult::Ok(data)),
                Err(e) => Outcome::Success(CacheResult::Err(e)),
            };
        } else {
            key(pattern, param)
        };
        match load(ctx.get_redis_connection(), key.clone()) {
            Ok(val) => {
                if val == String::default() {
                    match save_and_get(ctx, pattern, param, key.clone()).await {
                        Ok(data) => Outcome::Success(CacheResult::Ok(data)),
                        Err(e) => Outcome::Success(CacheResult::Err(e)),
                    }
                } else if is_obj(pattern) {
                    match serde_json::from_str(val.as_str()) {
                        Ok(data) => Outcome::Success(CacheResult::Ok(data)),
                        Err(e) => Outcome::Success(CacheResult::Err(CacheError::Deserialize(e))),
                    }
                } else {
                    Outcome::Success(CacheResult::Ok(json!(val)))
                }
            }
            Err(e) => Outcome::Success(CacheResult::Err(CacheError::Operate(e))),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CacheResult<MaybeLocked, CacheError> {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let address = match req.headers().get("account").next() {
            Some(data) => {
                let addr = remove_0x(data);
                match parse_addr(addr) {
                    Ok(_) => addr,
                    Err(e) => {
                        return Outcome::Success(CacheResult::Err(CacheError::ParseAddress(e)))
                    }
                }
            }
            None => return Outcome::Success(CacheResult::Err(CacheError::AccountIsNone)),
        };
        let ctx = req
            .guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient>>>()
            .await
            .unwrap();
        match hget(ctx.get_redis_connection(), hkey(), address.to_string()) {
            Ok(data) => match toml::from_str(data.as_str()) {
                Ok(account) => Outcome::Success(CacheResult::Ok(account)),
                Err(e) => Outcome::Success(CacheResult::Err(CacheError::TomlDe(e))),
            },
            Err(e) => Outcome::Success(CacheResult::Err(CacheError::Operate(e))),
        }
    }
}
