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

use anyhow::Context as Ctx;
use std::u64;

use crate::cita_cloud::account::Account;
use crate::cita_cloud::controller::{ControllerBehaviour, TransactionSenderBehaviour};
use crate::cita_cloud::executor::ExecutorBehaviour;
use crate::common::crypto::Address;
use crate::common::display::Display;
use crate::common::util::{hex_without_0x, parse_addr, parse_data, parse_value, remove_0x};
use crate::core::context::Context;
use crate::core::key_manager::{contract_key, CacheManager};
use crate::core::key_manager::{CacheBehavior, TxBehavior};
use crate::rest_api::common::{failure, success, CacheResult};
use crate::{ArrayLike, CacheConfig, ControllerClient, CryptoClient, EvmClient, ExecutorClient};
use anyhow::Result;
use cita_cloud_proto::blockchain::Transaction as CloudNormalTransaction;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct CreateContract<'r> {
    #[schema(
        example = "0x608060405234801561001057600080fd5b5060f58061001f6000396000f3006080604052600436106053576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff16806306661abd1460585780634f2be91f146080578063d826f88f146094575b600080fd5b348015606357600080fd5b50606a60a8565b6040518082815260200191505060405180910390f35b348015608b57600080fd5b50609260ae565b005b348015609f57600080fd5b5060a660c0565b005b60005481565b60016000808282540192505081905550565b600080819055505600a165627a7a72305820faa1d1f51d7b5ca2b200e0f6cdef4f2d7e44ee686209e300beb1146f40d32dee0029"
    )]
    pub data: &'r str,
    #[schema(example = "0x0")]
    pub value: Option<&'r str>,
    #[schema(example = 300000)]
    pub quota: Option<u64>,
    #[schema(example = 20)]
    pub block_count: Option<i64>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct SendTx<'r> {
    #[schema(example = "524268b46968103ce8323353dab16ae857f09a6f")]
    pub to: &'r str,
    #[schema(example = "0x4f2be91f")]
    pub data: Option<&'r str>,
    #[schema(example = "0x0")]
    pub value: Option<&'r str>,
    #[schema(example = 300000)]
    pub quota: Option<u64>,
    #[schema(example = 20)]
    pub block_count: Option<i64>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Call<'r> {
    #[deprecated]
    pub from: Option<&'r str>,
    #[schema(example = "b3eefbf4e5280217da74b83f316c5711827933a0")]
    pub to: &'r str,
    #[schema(example = "0x06661abd")]
    pub data: &'r str,
    #[schema(example = 0)]
    pub height: Option<u64>,
}

async fn get_contract_tx(
    result: CreateContract<'_>,
    client: ControllerClient,
) -> Result<CloudNormalTransaction> {
    let current = client.get_block_number(false).await?;
    let valid_until_block: u64 = (current as i64 + result.block_count.unwrap_or(20)) as u64;
    let to = Vec::new();
    let data = parse_data(result.data)?;
    let value = parse_value(result.value.unwrap_or("0x0"))?.to_vec();
    let quota = result.quota.unwrap_or(1073741824);
    let system_config = client
        .get_system_config()
        .await
        .context("failed to get system config")?;

    let tx = CloudNormalTransaction {
        version: system_config.version,
        to,
        data,
        value,
        nonce: rand::random::<u64>().to_string(),
        quota,
        valid_until_block,
        chain_id: system_config.chain_id,
    };

    Ok(tx)
}

pub async fn get_raw_tx(
    client: ControllerClient,
    tx: SendTx<'_>,
) -> Result<CloudNormalTransaction> {
    let current = client.get_block_number(false).await?;
    let valid_until_block: u64 = (current as i64 + tx.block_count.unwrap_or(20)) as u64;
    let to = parse_addr(tx.to)?.to_vec();
    let data = parse_data(tx.data.unwrap_or("0x"))?;
    let value = parse_value(tx.value.unwrap_or("0x0"))?.to_vec();
    let quota = tx.quota.unwrap_or(200000);
    let system_config = client
        .get_system_config()
        .await
        .context("failed to get system config")?;

    let tx = CloudNormalTransaction {
        version: system_config.version,
        to,
        data,
        value,
        nonce: rand::random::<u64>().to_string(),
        quota,
        valid_until_block,
        chain_id: system_config.chain_id,
    };

    Ok(tx)
}

///Create contract
#[post("/create", data = "<result>")]
#[utoipa::path(
post,
path = "/api/create",
request_body = CreateContract,
)]
pub async fn create(
    result: Json<CreateContract<'_>>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
    config: &State<CacheConfig>,
) -> Json<CacheResult<Value>> {
    let address = match parse_addr(config.account.as_str()) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    let account = Account::new(ctx.crypto.clone(), address);
    let tx = match get_contract_tx(result.0, ctx.controller.clone()).await {
        Ok(data) => data,
        Err(e) => return Json(failure(e)),
    };
    match ctx.controller.send_raw_tx(&account, tx.clone()).await {
        Ok(data) => {
            if let Err(e) = CacheManager::save_valid_until_block(
                hex_without_0x(data.as_slice()),
                tx.valid_until_block,
            ) {
                Json(failure(e))
            } else {
                Json(success(Value::String(data.display())))
            }
        }
        Err(e) => Json(failure(e)),
    }
}

///Send Transaction
#[post("/sendTx", data = "<result>")]
#[utoipa::path(
post,
path = "/api/sendTx",
request_body = SendTx,
)]
pub async fn send_tx(
    result: Json<SendTx<'_>>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
    config: &State<CacheConfig>,
) -> Json<CacheResult<Value>> {
    let address = match parse_addr(config.account.as_str()) {
        Ok(address) => address,
        Err(e) => return Json(failure(e)),
    };
    let account = Account::new(ctx.crypto.clone(), address);

    let raw_tx = match get_raw_tx(ctx.controller.clone(), result.clone().0).await {
        Ok(data) => data,
        Err(e) => return Json(failure(e)),
    };
    match ctx.controller.send_raw_tx(&account, raw_tx.clone()).await {
        Ok(data) => {
            if let Err(e) = CacheManager::save_valid_until_block(
                hex_without_0x(data.as_slice()),
                raw_tx.valid_until_block,
            ) {
                Json(failure(e))
            } else {
                Json(success(Value::String(data.display())))
            }
        }
        Err(e) => Json(failure(e)),
    }
}

fn call_param(
    result: Call<'_>,
    config: &State<CacheConfig>,
) -> Result<(Address, Address, Vec<u8>)> {
    let from = parse_addr(config.account.as_str())?;
    let to = parse_addr(result.to)?;
    let data = parse_data(result.data)?;
    Ok((from, to, data))
}
///Call
#[post("/call", data = "<result>")]
#[utoipa::path(
path = "/api/call",
post,
request_body = Call,
)]
pub async fn call(
    result: Json<Call<'_>>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
    config: &State<CacheConfig>,
) -> Json<CacheResult<Value>> {
    let (to_str, data_str, height, expire_time) = (
        remove_0x(result.to).to_string(),
        remove_0x(result.data).to_string(),
        result.height.unwrap_or(0),
        config.expire_time.unwrap() as usize,
    );
    let (from, to, data) = match call_param(result.0, config) {
        Ok(data) => data,
        Err(e) => return Json(failure(e)),
    };
    let key = contract_key(to_str, data_str, height);
    match CacheManager::load_or_query(
        key.clone(),
        expire_time,
        ctx.executor.call(from, to, data, height),
    )
    .await
    {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}
