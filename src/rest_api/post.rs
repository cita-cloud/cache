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

use std::{u64, usize};

use crate::cita_cloud::controller::TransactionSenderBehaviour;
use crate::cita_cloud::evm::EvmBehaviour;
use crate::cita_cloud::executor::ExecutorBehaviour;
use crate::cita_cloud::wallet::{MaybeLocked, MultiCryptoAccount};
use crate::common::context::BlockContext;
use crate::common::crypto::Address;
use crate::common::display::Display;
use crate::common::util::{hex_without_0x, parse_addr, parse_data, parse_value, remove_0x};
use crate::core::context::Context;
use crate::core::key_manager::{contract_key, CacheBehavior, CacheManager};
use crate::rest_api::common::{failure, success, CacheResult};
use crate::{
    ArrayLike, CacheConfig, ControllerClient, CryptoClient, EvmClient, ExecutorClient, Hash,
};
use anyhow::{anyhow, Result};
use cita_cloud_proto::blockchain::Transaction as CloudNormalTransaction;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

#[tonic::async_trait]
pub trait ToTx {
    async fn to(
        &self,
        account: &MultiCryptoAccount,
        evm: EvmClient,
    ) -> Result<CloudNormalTransaction>;

    async fn estimate_quota(
        &self,
        evm: EvmClient,
        from: Address,
        to: Address,
        data: Vec<u8>,
    ) -> Result<u64> {
        let bytes_quota = evm.estimate_quota(from, to, data).await?.bytes_quota;
        let quota = hex_without_0x(bytes_quota.as_slice());
        Ok(u64::from_str_radix(quota.as_str(), 16)?)
    }
}

#[derive(Clone)]
pub struct PackageTx {
    pub from: Address,
    pub to: Address,
    pub data: Vec<u8>,
    pub value: Vec<u8>,
    pub block_count: u64,
}

#[tonic::async_trait]
impl ToTx for PackageTx {
    async fn to(
        &self,
        _account: &MultiCryptoAccount,
        evm: EvmClient,
    ) -> Result<CloudNormalTransaction> {
        let current = BlockContext::current_cita_height()?;
        let valid_until_block: u64 = current + self.block_count;
        let to = self.to.clone().to_vec();
        let data = self.data.clone();
        let quota = self
            .estimate_quota(evm, self.from, self.to, self.data.clone())
            .await?;
        let value = self.value.clone();
        let system_config = BlockContext::system_config()?;
        let version = system_config.version;
        let chain_id = system_config.chain_id;
        let nonce = rand::random::<u64>().to_string();
        Ok(CloudNormalTransaction {
            version,
            to,
            data,
            value,
            nonce,
            quota,
            valid_until_block,
            chain_id,
        })
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct ChangeRole {
    #[schema(example = true)]
    pub is_master: bool,
}
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct CreateContract {
    #[schema(
        example = "0x608060405234801561001057600080fd5b5060f58061001f6000396000f3006080604052600436106053576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff16806306661abd1460585780634f2be91f146080578063d826f88f146094575b600080fd5b348015606357600080fd5b50606a60a8565b6040518082815260200191505060405180910390f35b348015608b57600080fd5b50609260ae565b005b348015609f57600080fd5b5060a660c0565b005b60005481565b60016000808282540192505081905550565b600080819055505600a165627a7a72305820faa1d1f51d7b5ca2b200e0f6cdef4f2d7e44ee686209e300beb1146f40d32dee0029"
    )]
    pub data: String,
    #[schema(example = "0x0")]
    pub value: Option<String>,
    #[schema(example = 20)]
    pub block_count: Option<i64>,
    pub local_execute: bool,
}

impl Default for CreateContract {
    fn default() -> Self {
        Self {
            data: "0x608060405234801561001057600080fd5b5060f58061001f6000396000f3006080604052600436106053576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff16806306661abd1460585780634f2be91f146080578063d826f88f146094575b600080fd5b348015606357600080fd5b50606a60a8565b6040518082815260200191505060405180910390f35b348015608b57600080fd5b50609260ae565b005b348015609f57600080fd5b5060a660c0565b005b60005481565b60016000808282540192505081905550565b600080819055505600a165627a7a72305820faa1d1f51d7b5ca2b200e0f6cdef4f2d7e44ee686209e300beb1146f40d32dee0029".to_string(),
            value: Some("0x0".to_string()),
            block_count: Some(20),
            local_execute: true,
        }
    }
}
#[tonic::async_trait]
impl ToTx for CreateContract {
    async fn to(
        &self,
        account: &MultiCryptoAccount,
        evm: EvmClient,
    ) -> Result<CloudNormalTransaction> {
        let current = BlockContext::current_cita_height()?;
        let valid_until_block: u64 = (current as i64 + self.block_count.unwrap_or_default()) as u64;
        let to = Vec::new();
        let data = parse_data(self.data.clone().as_str())?;
        let quota = self
            .estimate_quota(
                evm,
                Address::try_from_slice(account.address())?,
                Address::default(),
                data.clone(),
            )
            .await?;
        let value = parse_value(self.value.clone().unwrap_or_default().as_str())?.to_vec();
        let system_config = BlockContext::system_config()?;
        let version = system_config.version;
        let chain_id = system_config.chain_id;
        let nonce = rand::random::<u64>().to_string();
        Ok(CloudNormalTransaction {
            version,
            to,
            data,
            value,
            nonce,
            quota,
            valid_until_block,
            chain_id,
        })
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct SendTx {
    #[schema(example = "0x524268b46968103ce8323353dab16ae857f09a6f")]
    pub to: String,
    #[schema(example = "0x4f2be91f")]
    pub data: Option<String>,
    #[schema(example = "0x0")]
    pub value: Option<String>,
    #[schema(example = 20)]
    pub block_count: Option<i64>,
    pub local_execute: bool,
}

impl Default for SendTx {
    fn default() -> Self {
        Self {
            to: "0x524268b46968103ce8323353dab16ae857f09a6f".to_string(),
            data: Some("0x4f2be91f".to_string()),
            value: Some("0x0".to_string()),
            block_count: Some(20),
            local_execute: true,
        }
    }
}

#[tonic::async_trait]
impl ToTx for SendTx {
    async fn to(
        &self,
        account: &MultiCryptoAccount,
        evm: EvmClient,
    ) -> Result<CloudNormalTransaction> {
        let current = BlockContext::current_cita_height()?;
        let valid_until_block: u64 = (current as i64 + self.block_count.unwrap_or_default()) as u64;
        let to = parse_addr(self.to.clone().as_str())?;
        let data = parse_data(self.data.clone().unwrap_or_default().as_str())?;
        let value = parse_value(self.value.clone().unwrap_or_default().as_str())?.to_vec();
        let quota = self
            .estimate_quota(
                evm,
                Address::try_from_slice(account.address())?,
                to,
                data.clone(),
            )
            .await?;

        let system_config = BlockContext::system_config()?;
        let version = system_config.version;
        let chain_id = system_config.chain_id;
        let nonce = rand::random::<u64>().to_string();
        Ok(CloudNormalTransaction {
            version,
            to: to.to_vec(),
            data,
            value,
            nonce,
            quota,
            valid_until_block,
            chain_id,
        })
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Call {
    pub from: Option<String>,
    #[schema(example = "0xb3eefbf4e5280217da74b83f316c5711827933a0")]
    pub to: String,
    #[schema(example = "0x06661abd")]
    pub data: String,
    #[schema(example = 0)]
    pub height: Option<u64>,
}

impl Default for Call {
    fn default() -> Self {
        Self {
            from: None,
            to: "0xb3eefbf4e5280217da74b83f316c5711827933a0".to_string(),
            data: "0x06661abd".to_string(),
            height: Some(0),
        }
    }
}

async fn create_contract(
    evm: EvmClient,
    controller: ControllerClient,
    create_contract: CreateContract,
) -> Result<Hash> {
    let maybe: MaybeLocked = BlockContext::current_account()?;
    let account = maybe.unlocked()?;
    let tx = create_contract.to(account, evm.clone()).await?;
    controller
        .send_raw_tx(account, tx, create_contract.local_execute)
        .await
}

///Change role online
#[post("/change-role", data = "<result>")]
#[utoipa::path(
post,
path = "/api/change-role",
request_body = ChangeRole,
)]
pub async fn change_role(result: Json<ChangeRole>) -> Json<CacheResult<Value>> {
    match BlockContext::change_role(result.is_master) {
        Ok(data) => Json(success(json!(data))),
        Err(e) => Json(failure(anyhow!(e))),
    }
}

///Create contract
#[post("/create", data = "<result>")]
#[utoipa::path(
post,
path = "/api/create",
request_body = CreateContract,
)]
pub async fn create(
    result: Json<CreateContract>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    if let Ok(true) = BlockContext::is_master() {
        match create_contract(ctx.local_evm.clone(), ctx.controller.clone(), result.0).await {
            Ok(data) => Json(success(data.to_json())),
            Err(e) => Json(failure(e)),
        }
    } else {
        Json(failure(anyhow!("only master can do this!")))
    }
}

async fn create_tx(evm: EvmClient, controller: ControllerClient, send_tx: SendTx) -> Result<Hash> {
    let maybe: MaybeLocked = BlockContext::current_account()?;
    let account = maybe.unlocked()?;
    let tx = send_tx.to(account, evm.clone()).await?;

    controller
        .send_raw_tx(account, tx, send_tx.local_execute)
        .await
}

///Send Transaction
#[post("/sendTx", data = "<result>")]
#[utoipa::path(
post,
path = "/api/sendTx",
request_body = SendTx,
)]
pub async fn send_tx(
    result: Json<SendTx>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
) -> Json<CacheResult<Value>> {
    if let Ok(true) = BlockContext::is_master() {
        match create_tx(ctx.local_evm.clone(), ctx.controller.clone(), result.0).await {
            Ok(data) => Json(success(data.to_json())),
            Err(e) => Json(failure(e)),
        }
    } else {
        Json(failure(anyhow!("only master can do this!")))
    }
}

async fn call_or_load(
    result: Call,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
    config: &State<CacheConfig>,
) -> Result<Value> {
    let key = contract_key(
        remove_0x(result.to.as_str()).to_string(),
        remove_0x(result.data.as_str()).to_string(),
        result.height.unwrap_or_default(),
    );
    let from = parse_addr(config.account.as_str())?;
    let to = parse_addr(result.to.as_str())?;
    let data = parse_data(result.data.as_str())?;
    let height = result.height.unwrap_or_default();
    let expire_time = config.expire_time.unwrap() as usize;
    CacheManager::load_or_query(
        key,
        expire_time,
        ctx.local_executor.call(from, to, data, height),
    )
    .await
}
///Call
#[post("/call", data = "<result>")]
#[utoipa::path(
path = "/api/call",
post,
request_body = Call,
)]
pub async fn call(
    result: Json<Call>,
    ctx: &State<Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>>,
    config: &State<CacheConfig>,
) -> Json<CacheResult<Value>> {
    match call_or_load(result.0, ctx, config).await {
        Ok(data) => Json(success(data)),
        Err(e) => Json(failure(e)),
    }
}
