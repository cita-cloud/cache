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
use utoipa::Component;

use crate::context::Context;
use crate::core::account::Account;
use crate::core::controller::{ControllerBehaviour, TransactionSenderBehaviour};
use crate::display::Display;
use crate::rest_api::common::{failure, success, CacheResult};
use crate::util::{parse_addr, parse_data, parse_value};
use crate::{ArrayLike, ControllerClient, CryptoClient, EvmClient, ExecutorClient};
use anyhow::Result;
use cita_cloud_proto::blockchain::Transaction as CloudNormalTransaction;
use rocket::serde::json::Json;
use serde::Deserialize;
use serde_json::Value;

#[derive(Component, Deserialize)]
#[serde(crate = "rocket::serde")]
#[component(example = json!({"data": "", "value": "0x0", "quota": 1073741824}))]
pub struct CreateContract<'r> {
    pub data: &'r str,
    pub value: Option<&'r str>,
    pub quota: Option<u64>,
    pub valid_until_block: Option<i64>,
}

#[derive(Component, Deserialize)]
#[serde(crate = "rocket::serde")]
#[component(example = json!({"to": "524268b46968103ce8323353dab16ae857f09a6f", "data": "0x", "value": "0x0", "quota": 1073741824}))]
pub struct SendTx<'r> {
    pub to: &'r str,
    pub data: Option<&'r str>,
    pub value: Option<&'r str>,
    pub quota: Option<u64>,
    pub valid_until_block: Option<i64>,
}

async fn get_contract_tx(
    result: CreateContract<'_>,
    client: ControllerClient,
) -> Result<CloudNormalTransaction> {
    let current = client.get_block_number(false).await?;
    let valid_until_block: u64 = (current as i64 + result.valid_until_block.unwrap_or(20)) as u64;
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
    let valid_until_block: u64 = (current as i64 + 20) as u64;
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

pub async fn new_raw_tx(
    client: ControllerClient,
    tx: CloudNormalTransaction,
) -> Result<CloudNormalTransaction> {
    let current = client.get_block_number(false).await?;
    let valid_until_block: u64 = (current as i64 + 20) as u64;
    let to = tx.to;
    let data = tx.data;
    let value = tx.value;
    let quota = tx.quota;
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
    ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    let account = Account::new(ctx.crypto.clone());
    let tx = match get_contract_tx(result.0, ctx.controller.clone()).await {
        Ok(data) => data,
        Err(e) => return Json(failure(e)),
    };
    match ctx.controller.send_raw_tx(&account, tx).await {
        Ok(data) => Json(success(Value::String(data.display()))),
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
    ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Json<CacheResult<Value>> {
    let account = Account::new(ctx.crypto.clone());

    let raw_tx = match get_raw_tx(ctx.controller.clone(), result.0).await {
        Ok(data) => data,
        Err(e) => return Json(failure(e)),
    };
    match ctx.controller.send_raw_tx(&account, raw_tx).await {
        Ok(data) => Json(success(Value::String(data.display()))),
        Err(e) => Json(failure(e)),
    }
}
