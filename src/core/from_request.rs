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

use crate::common::display::Display;
use crate::common::util::{parse_addr, parse_hash, parse_u64, remove_0x};
use rocket::{Request, State};

use crate::cita_cloud::controller::ControllerBehaviour;
use crate::cita_cloud::evm::EvmBehaviour;
use crate::common::error::CacheError;
use crate::core::key_manager::expire;
use crate::core::key_manager::{key, key_without_param};
use crate::redis::{get, set_ex, ttl};
use crate::rest_api::common::{failure, success, CacheResult};
use crate::{CacheConfig, Context, ControllerClient, CryptoClient, EvmClient, ExecutorClient};
use anyhow::Result;
use rocket::http::Method::Get;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use serde_json::{json, Value};
use std::string::String;

fn get_param<'r>(req: &'r Request<'_>, index: usize) -> &'r str {
    if let Some(val) = req.param(index) {
        val.ok().unwrap()
    } else {
        ""
    }
}

fn path_count(req: &Request) -> usize {
    req.uri().path().split('/').count()
}

fn with_param(req: &Request) -> bool {
    path_count(req) == vec!["", "api", "{query-name}", "{param}"].len()
}

async fn query_and_load(
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
    path: &str,
    param: String,
    key: String,
    expire_time: usize,
) -> Result<Value> {
    match path {
        "block-number" => {
            let block_number = ctx.controller.get_block_number(false).await?;
            set_ex(key, block_number, expire_time)?;
            Ok(json!(block_number))
        }
        "peers-count" => {
            let peer_count = ctx.controller.get_peer_count().await?;
            set_ex(key, peer_count, expire_time)?;
            Ok(json!(peer_count))
        }
        "version" => {
            let version = ctx.controller.get_version().await?;
            set_ex(key, version.clone(), expire_time)?;
            Ok(json!(version))
        }
        "peers-info" => {
            let info = ctx.controller.get_peers_info().await?;
            set_ex(key, info.display(), expire_time)?;
            Ok(info.to_json())
        }
        "system-config" => {
            let config = ctx.controller.get_system_config().await?;
            set_ex(key, config.display(), expire_time)?;
            Ok(config.to_json())
        }
        "abi" => {
            let data = parse_addr(param.as_str())?;
            let abi = ctx.evm.get_abi(data).await?;
            set_ex(key, abi.display(), expire_time)?;
            Ok(abi.to_json())
        }
        "account-nonce" => {
            let data = parse_addr(param.as_str())?;
            let nonce = ctx.evm.get_tx_count(data).await?;
            set_ex(key, nonce.display(), expire_time)?;
            Ok(nonce.to_json())
        }
        "balance" => {
            let data = parse_addr(param.as_str())?;
            let balance = ctx.evm.get_balance(data).await?;
            set_ex(key, balance.display(), expire_time)?;
            Ok(balance.to_json())
        }
        "code" => {
            let data = parse_addr(param.as_str())?;
            let code = ctx.evm.get_code(data).await?;
            set_ex(key, code.display(), expire_time)?;
            Ok(code.to_json())
        }
        "block-hash" => {
            let data = parse_u64(param.as_str())?;
            let hash = ctx.controller.get_block_hash(data).await?;
            set_ex(key, hash.display(), expire_time)?;
            Ok(hash.to_json())
        }
        "receipt" => {
            let data = parse_hash(param.as_str())?;
            let receipt = ctx.evm.get_receipt(data).await?;
            set_ex(key, receipt.display(), expire_time)?;
            Ok(receipt.to_json())
        }
        "tx" => {
            let data = parse_hash(param.as_str())?;
            let tx = ctx.controller.get_tx(data).await?;
            set_ex(key, tx.display(), expire_time)?;
            Ok(tx.to_json())
        }
        "block" => {
            if let Ok(data) = parse_u64(param.as_str()) {
                let block = ctx.controller.get_block_by_number(data).await?;
                set_ex(key, block.display(), expire_time)?;
                Ok(block.to_json())
            } else {
                match parse_hash(param.as_str()) {
                    Ok(data) => {
                        let block = ctx.controller.get_block_by_hash(data).await?;
                        set_ex(key, block.display(), expire_time)?;
                        Ok(block.to_json())
                    }
                    Err(e) => Err(e),
                }
            }
        }
        _ => Ok(Value::Null),
    }
}

async fn try_load(
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
    pattern: &str,
    param: String,
    expire_time: usize,
) -> Result<CacheResult<Value>> {
    let (key, param) = (key(pattern.to_string(), param.clone()), param);
    match ttl(key.clone()) {
        Ok(-1) | Ok(-2) => {
            let data = query_and_load(ctx, pattern, param, key.clone(), expire_time).await?;
            Ok(success(data))
        }
        Ok(_) => {
            expire(key.clone(), expire_time)?;
            let val = get(key.clone())?;
            match serde_json::from_str(val.as_str()) {
                Ok(data) => Ok(success(data)),
                Err(_) => Ok(success(Value::String(val))),
            }
        }
        Err(e) => Err(anyhow::Error::from(e)),
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient> {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let ctx = req
            .guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>>()
            .await
            .unwrap();
        Outcome::Success(Context {
            controller: ctx.controller.clone(),
            executor: ctx.executor.clone(),
            evm: ctx.evm.clone(),
            crypto: ctx.crypto.clone(),
            redis_pool: ctx.redis_pool.clone(),
        })
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CacheResult<Value> {
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
            .guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>>()
            .await
            .unwrap();
        let config = req.guard::<&State<CacheConfig>>().await.unwrap();
        let pattern = &get_param(req, 0)["get-".len()..];
        let param = remove_0x(get_param(req, 1));

        if !with_param(req) {
            match query_and_load(
                ctx,
                pattern,
                param.to_string(),
                key_without_param(pattern),
                config.expire_time.unwrap() as usize,
            )
            .await
            {
                Ok(data) => Outcome::Success(success(data)),
                Err(e) => Outcome::Success(failure(e)),
            }
        } else {
            match try_load(
                ctx,
                pattern,
                param.to_string(),
                config.expire_time.unwrap() as usize,
            )
            .await
            {
                Ok(data) => Outcome::Success(data),
                Err(e) => Outcome::Success(failure(e)),
            }
        }
    }
}
