use anyhow::Context as Ctx;
use utoipa::Component;

use crate::context::Context;
use crate::core::account::Account;
use crate::core::controller::{ControllerBehaviour, TransactionSenderBehaviour};
use crate::display::Display;
use crate::error::CacheError;
use crate::rest_api::common::{fail, success, QueryResult};
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
    result: Json<CreateContract<'_>>,
    ctx: &Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Result<CloudNormalTransaction> {
    let current = ctx.controller.get_block_number(false).await?;
    let valid_until_block: u64 = (current as i64 + result.valid_until_block.unwrap_or(20)) as u64;
    let to = Vec::new();
    let data = parse_data(result.data)?;
    let value = parse_value(result.value.unwrap_or("0x0"))?.to_vec();
    let quota = result.quota.unwrap_or(1073741824);
    let system_config = ctx
        .controller
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

async fn get_raw_tx(
    result: Json<SendTx<'_>>,
    ctx: &Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
) -> Result<CloudNormalTransaction> {
    let current = ctx.controller.get_block_number(false).await?;
    let valid_until_block: u64 = (current as i64 + result.valid_until_block.unwrap_or(20)) as u64;
    let to = parse_addr(result.to)?.to_vec();
    let data = parse_data(result.data.unwrap_or("0x"))?;
    let value = parse_value(result.value.unwrap_or("0x0"))?.to_vec();
    let quota = result.quota.unwrap_or(200000);
    let system_config = ctx
        .controller
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
) -> Json<QueryResult<Value>> {
    let account = Account::new(ctx.crypto.clone());
    let raw = match get_contract_tx(result, &ctx).await {
        Ok(data) => data,
        Err(e) => return fail(CacheError::ParseAddress(e)),
    };
    match ctx.controller.send_raw_tx(&account, raw).await {
        Ok(data) => success(Value::String(data.display())),
        Err(e) => fail(CacheError::ParseAddress(e)),
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
) -> Json<QueryResult<Value>> {
    let account = Account::new(ctx.crypto.clone());

    let raw_tx = match get_raw_tx(result, &ctx).await {
        Ok(data) => data,
        Err(e) => return fail(CacheError::ParseAddress(e)),
    };
    match ctx.controller.send_raw_tx(&account, raw_tx).await {
        Ok(data) => success(Value::String(data.display())),
        Err(e) => fail(CacheError::ParseAddress(e)),
    }
}
