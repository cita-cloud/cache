use utoipa::Component;

use crate::context::Context;
use crate::core::account::{Account, CryptoType, MaybeLocked, MultiCryptoAccount};
use crate::core::controller::{ControllerBehaviour, TransactionSenderBehaviour};
use crate::crypto::{EthCrypto, SmCrypto};
use crate::display::Display;
use crate::error::CacheError;
use crate::from_request::CacheResult;
use crate::redis::hset;
use crate::rest_api::common::{fail, success, QueryResult};
use crate::{ControllerClient, EvmClient, ExecutorClient};
use rocket::serde::json::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::util::{hex, hex_without_0x, hkey, parse_data, parse_value};

#[derive(Component, Deserialize)]
#[serde(crate = "rocket::serde")]
#[component(example = json!({"data": "", "value": "0x0", "quota": 1073741824, "valid_until_block": 95}))]
pub struct CreateContract<'r> {
    pub data: &'r str,
    pub value: Option<&'r str>,
    pub quota: Option<u64>,
    pub valid_until_block: Option<i64>,
}

#[derive(Component, Deserialize)]
#[serde(crate = "rocket::serde")]
#[component(example = json!({"crypto_type": "SM"}))]
pub struct GenerateAccount {
    pub crypto_type: CryptoType,
}

///Create contract
#[post("/create", data = "<result>")]
#[utoipa::path(
post,
path = "/api/create",
request_body = CreateContract,
params(
("account" = String, header, description = "context account"),
),
)]
pub async fn create(
    account: CacheResult<MaybeLocked, CacheError>,
    result: Json<CreateContract<'_>>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<Value>> {
    let account = match account {
        CacheResult::Ok(data) => data,
        CacheResult::Err(e) => return fail(e),
    };
    let current = match ctx.controller.get_block_number(false).await {
        Ok(data) => data,
        Err(detail) => {
            return fail(CacheError::QueryCitaCloud {
                query_type: "get block number".to_string(),
                detail,
            })
        }
    };
    let height: u64 = (current as i64 + result.valid_until_block.unwrap_or(95)) as u64;
    let to = Vec::new();
    let data = match parse_data(result.data) {
        Ok(data) => data,
        Err(e) => return fail(CacheError::ParseAddress(e)),
    };
    let value = match parse_value(result.value.unwrap_or("0x0")) {
        Ok(v) => v,
        Err(e) => return fail(CacheError::ParseAddress(e)),
    }
    .to_vec();
    let quota = result.quota.unwrap_or(1073741824);
    match ctx
        .controller
        .send_tx(account.unlocked().unwrap(), to, data, value, quota, height)
        .await
    {
        Ok(data) => success(Value::String(data.display())),
        Err(detail) => fail(CacheError::QueryCitaCloud {
            query_type: "send tx".to_string(),
            detail,
        }),
    }
}

///Generate account
#[post("/account/generate", data = "<result>")]
#[utoipa::path(
post,
path = "/api/account/generate",
request_body = GenerateAccount,
)]
pub async fn generate_account(
    result: Json<GenerateAccount>,
    ctx: Context<ControllerClient, ExecutorClient, EvmClient>,
) -> Json<QueryResult<Value>> {
    let account: MultiCryptoAccount = match result.crypto_type {
        CryptoType::Sm => Account::<SmCrypto>::generate().into(),
        CryptoType::Eth => Account::<EthCrypto>::generate().into(),
    };
    let maybe_locked: MaybeLocked = account.into();
    let content = match toml::to_string_pretty(&maybe_locked) {
        Ok(data) => data,
        Err(e) => return fail(CacheError::TomlSer(e)),
    };
    match hset(
        ctx.get_redis_connection(),
        hkey(),
        hex_without_0x(maybe_locked.address()),
        content,
    ) {
        Ok(_) => success(
            json!({"crypto_type": maybe_locked.crypto_type(), "address": hex(maybe_locked.address()), "public_key": hex(maybe_locked.public_key())}),
        ),
        Err(e) => fail(CacheError::Operate(e)),
    }
}
