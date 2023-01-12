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

use crate::cita_cloud::controller::SignerBehaviour;
use crate::cita_cloud::executor::ExecutorBehaviour;
use crate::cita_cloud::wallet::MaybeLocked;
use crate::common::constant::{
    local_executor, ADMIN_ACCOUNT, BLOCK_NUMBER, CURRENT_BLOCK_HASH, CURRENT_BLOCK_NUMBER,
    SYSTEM_CONFIG,
};
use crate::common::util::hex;
use crate::common::util::timestamp;
use crate::core::key_manager::key_without_param;
use crate::redis::{get_num, incr, set};
use crate::{exists, get};
use anyhow::{anyhow, Result};
use cita_cloud_proto::{
    blockchain::{Block, BlockHeader, RawTransactions},
    controller::SystemConfig,
};
use cloud_util::clean_0x;
use prost::Message;
#[tonic::async_trait]
pub trait LocalBehaviour {
    async fn get_current_height() -> Result<u64>;

    async fn get_current_block_hash() -> Result<Vec<u8>>;

    fn increase_block_height() -> Result<u64>;

    fn current_height_exist() -> Result<bool>;

    fn current_block_hash_exist() -> Result<bool>;

    fn set_current_block_hash(hash: String) -> Result<String>;

    async fn commit_genesis_block() -> Result<()>;
}

#[tonic::async_trait]
pub trait SystemParamBehaviour {
    async fn get_current_height() -> Result<u64>;

    async fn get_current_block_hash() -> Result<Vec<u8>>;

    fn increase_block_height() -> Result<u64>;

    fn current_height_exist() -> Result<bool>;

    fn current_block_hash_exist() -> Result<bool>;

    fn set_current_block_hash(hash: String) -> Result<String>;

    async fn commit_genesis_block() -> Result<()>;
}

pub struct BlockContext;

impl BlockContext {
    pub fn current_height() -> Result<u64> {
        let current = get::<u64>(key_without_param(BLOCK_NUMBER.to_string()))?;
        Ok(current)
    }

    pub fn system_config() -> Result<SystemConfig> {
        let system_config = get::<Vec<u8>>(key_without_param(SYSTEM_CONFIG.to_string()))?;
        let config: SystemConfig = Message::decode(system_config.as_slice())?;
        Ok(config)
    }

    pub fn current_account() -> Result<MaybeLocked> {
        let account_str = get::<String>(key_without_param(ADMIN_ACCOUNT.to_string()))?;
        let maybe: MaybeLocked = toml::from_str::<MaybeLocked>(account_str.as_str())?;
        Ok(maybe)
    }
}

#[tonic::async_trait]
impl LocalBehaviour for BlockContext {
    fn increase_block_height() -> Result<u64> {
        Ok(incr(key_without_param(CURRENT_BLOCK_NUMBER.to_string()))?)
    }

    async fn get_current_height() -> Result<u64> {
        let key = key_without_param(CURRENT_BLOCK_NUMBER.to_string());
        if exists(key.clone())? {
            Ok(get_num(key.clone())?)
        } else {
            Self::commit_genesis_block().await?;
            Ok(get_num(key.clone())?)
        }
    }

    fn current_height_exist() -> Result<bool> {
        Ok(exists(key_without_param(CURRENT_BLOCK_NUMBER.to_string()))?)
    }

    fn current_block_hash_exist() -> Result<bool> {
        Ok(exists(key_without_param(CURRENT_BLOCK_HASH.to_string()))?)
    }

    async fn get_current_block_hash() -> Result<Vec<u8>> {
        let key = key_without_param(CURRENT_BLOCK_HASH.to_string());
        if exists(key.clone())? {
            let hash_str: String = get(key.clone())?;
            Ok(hex::decode(clean_0x(hash_str.as_str()))?)
        } else {
            Self::commit_genesis_block().await?;
            let hash_str: String = get(key.clone())?;
            Ok(hex::decode(clean_0x(hash_str.as_str()))?)
        }
    }

    fn set_current_block_hash(hash: String) -> Result<String> {
        let key = key_without_param(CURRENT_BLOCK_HASH.to_string());
        Ok(set(key, hash)?)
    }

    async fn commit_genesis_block() -> Result<()> {
        let maybe: MaybeLocked = Self::current_account()?;
        let account = maybe.unlocked()?;
        let genesis_header = BlockHeader {
            prevhash: vec![0u8; 32],
            timestamp: timestamp(),
            height: 0,
            transactions_root: vec![0u8; 32],
            proposer: vec![0u8; 32],
        };

        let res = local_executor()
            .exec(Block {
                version: 0,
                header: Some(genesis_header.clone()),
                body: Some(RawTransactions { body: Vec::new() }),
                proof: vec![],
                state_root: vec![],
            })
            .await?;
        if let Some(status) = res.status {
            if status.code == 0 {
                let mut block_header_bytes = Vec::with_capacity(genesis_header.encoded_len());
                genesis_header
                    .encode(&mut block_header_bytes)
                    .expect("encode block header failed");
                let block_hash = account.hash(block_header_bytes.as_slice());
                let hash_str = hex(block_hash.as_slice());
                println!("current block hash: {}", hash_str);

                Self::increase_block_height()?;
                Self::set_current_block_hash(hash_str)?;
                Ok(())
            } else {
                Err(anyhow!("execute local block failed!"))
            }
        } else {
            Err(anyhow!("commit genesis block failed!"))
        }
    }
}
