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

use crate::cita_cloud::controller::{ControllerBehaviour, SignerBehaviour};
use crate::cita_cloud::executor::ExecutorBehaviour;
use crate::cita_cloud::wallet::MaybeLocked;
use crate::common::constant::{controller, local_executor, ADMIN_ACCOUNT};
use crate::common::util::{hex, parse_data, timestamp};
use crate::core::key_manager::{
    admin_account_key, cita_cloud_block_number_key, key_without_param, rollup_write_enable,
    system_config_key,
};
use crate::core::key_manager::{current_batch_number, current_fake_block_hash};
use crate::redis::{set, Connection};
use crate::{config, exists, get, incr_one, CacheBehavior, CacheManager, CryptoType, KEY_PAIR};
use anyhow::{anyhow, Result};
use cita_cloud_proto::{
    blockchain::{Block, BlockHeader, RawTransaction, RawTransactions},
    controller::SystemConfig,
};
use cloud_util::unix_now;
use prost::Message;
use std::cmp;

#[tonic::async_trait]
pub trait LocalBehaviour {
    async fn set_up(con: &mut Connection) -> Result<()>;
    async fn timing_update(con: &mut Connection, expire_time: usize) -> Result<()>;

    async fn get_batch_number(con: &mut Connection) -> Result<u64>;

    async fn get_fake_block_hash(con: &mut Connection) -> Result<Vec<u8>>;

    async fn fake_block(
        con: &mut Connection,
        proposer: Vec<u8>,
        tx_list: Vec<RawTransaction>,
    ) -> Result<Block>;

    async fn commit_genesis_block(con: &mut Connection) -> Result<()>;
}

pub struct BlockContext;

impl BlockContext {
    fn create_admin_account(con: &mut Connection, crypto_type: CryptoType) -> Result<()> {
        let account_key = key_without_param(ADMIN_ACCOUNT.to_string());
        let (private_key, account_str) = match crypto_type {
            CryptoType::Sm => ("0x67aef6c49c853f6022e27d2099aeeaf732f41a76770855b3837b8c000b4b945e", "crypto_type = 'SM'\naddress = '0x36fbd539ca0ade15bcfc453a79f5f33450749b51'\npublic_key = '0xc4162ace569f6e82be31b47b0ba4aa0cd991ef10dc98397aedc16fecf86636e4e6154649fe07671edc7c1965e87eefa1bd65018fcb39f16729cd246250b1116e'\nsecret_key = '0x67aef6c49c853f6022e27d2099aeeaf732f41a76770855b3837b8c000b4b945e'\n"),
            CryptoType::Eth => ("0xe230059d4c1be58ba1e4f063e977ec1c6d1f8d754a2e30a36af297619379230e", "crypto_type = 'ETH'\naddress = '0x82c00d52dc6b9857352c5632496fbf0b3ea05c5f'\npublic_key = '0xefebaa703b5ba34801c581139c358420196e0e179acdc532513e5098ebf7b28605fa014798db38260dbf788f8bd45b885265254cf932e90caf0b1e9b5ebf82de'\nsecret_key = '0xe230059d4c1be58ba1e4f063e977ec1c6d1f8d754a2e30a36af297619379230e'\n"),
        };
        let input = parse_data(private_key)?;
        let keypair = efficient_sm2::KeyPair::new(input.as_slice()).unwrap();
        if let Err(e) = KEY_PAIR.set(keypair) {
            panic!("store key pair error: {e}");
        }
        set(con, account_key, account_str.to_string())?;
        Ok(())
    }
    pub fn current_cita_height(con: &mut Connection) -> Result<u64> {
        let current = get::<u64>(con, cita_cloud_block_number_key())?;
        Ok(current)
    }

    pub fn system_config(con: &mut Connection) -> Result<SystemConfig> {
        let system_config = get::<Vec<u8>>(con, system_config_key())?;
        let config: SystemConfig = Message::decode(system_config.as_slice())?;
        Ok(config)
    }

    pub fn current_account(con: &mut Connection) -> Result<MaybeLocked> {
        let account_str = get::<String>(con, admin_account_key())?;
        let maybe: MaybeLocked = toml::from_str::<MaybeLocked>(account_str.as_str())?;
        Ok(maybe)
    }

    fn genesis_block() -> Block {
        let genesis_header = BlockHeader {
            prevhash: vec![0u8; 32],
            timestamp: timestamp(),
            height: 0,
            transactions_root: vec![0u8; 32],
            proposer: vec![0u8; 32],
        };
        Block {
            version: 0,
            header: Some(genesis_header),
            body: Some(RawTransactions { body: Vec::new() }),
            proof: vec![],
            state_root: vec![],
        }
    }

    pub fn increase_batch_number(con: &mut Connection) -> Result<u64> {
        Ok(incr_one(con, current_batch_number())?)
    }
    pub fn set_fake_block_hash(con: &mut Connection, hash: Vec<u8>) -> Result<String> {
        Ok(set(con, current_fake_block_hash(), hash)?)
    }

    pub fn step_next(con: &mut Connection, block_hash: Vec<u8>) -> Result<()> {
        Self::increase_batch_number(con)?;
        Self::set_fake_block_hash(con, block_hash)?;
        Ok(())
    }

    pub fn change_role(con: &mut Connection, is_master: bool) -> Result<()> {
        let key = rollup_write_enable();
        match is_master {
            true => {
                set(con, key, 1)?;
                Ok(())
            }
            false => {
                set(con, key, 0)?;
                Ok(())
            }
        }
    }

    pub fn is_master(con: &mut Connection) -> Result<bool> {
        let key = rollup_write_enable();
        Ok(exists(con, key.clone())? && get::<u64>(con, key)? == 1)
    }

    fn is_restart(con: &mut Connection) -> Result<bool> {
        Ok(exists(con, rollup_write_enable())?)
    }
}

#[tonic::async_trait]
impl LocalBehaviour for BlockContext {
    async fn set_up(con: &mut Connection) -> Result<()> {
        let config = config();
        Self::timing_update(con, config.expire_time.unwrap_or_default() as usize).await?;
        Self::create_admin_account(con, config.crypto_type)?;
        if !Self::is_restart(con)? {
            Self::change_role(con, config.is_master)?;
        }
        Ok(())
    }

    async fn timing_update(con: &mut Connection, expire_time: usize) -> Result<()> {
        let key = cita_cloud_block_number_key();
        let num = if exists(con, key.clone())? {
            get(con, key)?
        } else {
            0
        };
        CacheManager::set_ex(
            con,
            cita_cloud_block_number_key(),
            cmp::max(controller().get_block_number(false).await?, num),
            expire_time * 2,
        )?;
        let sys_config = controller().get_system_config().await?;
        let mut sys_config_bytes = Vec::with_capacity(sys_config.encoded_len());
        sys_config
            .encode(&mut sys_config_bytes)
            .expect("encode system config failed");
        CacheManager::set_ex(con, system_config_key(), sys_config_bytes, expire_time * 2)?;
        Ok(())
    }

    async fn get_batch_number(con: &mut Connection) -> Result<u64> {
        let key = current_batch_number();
        if exists(con, key.clone())? {
            Ok(get::<u64>(con, key.clone())?)
        } else {
            Self::commit_genesis_block(con).await?;
            Ok(get::<u64>(con, key.clone())?)
        }
    }

    async fn get_fake_block_hash(con: &mut Connection) -> Result<Vec<u8>> {
        let key = current_fake_block_hash();
        if exists(con, key.clone())? {
            Ok(get(con, key.clone())?)
        } else {
            Self::commit_genesis_block(con).await?;
            Ok(get(con, key.clone())?)
        }
    }

    async fn fake_block(
        con: &mut Connection,
        proposer: Vec<u8>,
        tx_list: Vec<RawTransaction>,
    ) -> Result<Block> {
        let header = BlockHeader {
            prevhash: Self::get_fake_block_hash(con).await?,
            timestamp: unix_now(),
            height: Self::get_batch_number(con).await?,
            transactions_root: vec![0u8; 32],
            proposer,
        };

        Ok(Block {
            version: 0,
            header: Some(header),
            body: Some(RawTransactions { body: tx_list }),
            proof: Vec::new(),
            state_root: Vec::new(),
        })
    }

    async fn commit_genesis_block(con: &mut Connection) -> Result<()> {
        let maybe: MaybeLocked = Self::current_account(con)?;
        let account = maybe.unlocked()?;
        let block = Self::genesis_block();
        let res = local_executor().exec(block.clone()).await?;
        if let Some(status) = res.status {
            if status.code == 0 {
                let genesis_header = block.header.expect("get genesis header failed!");
                let mut block_header_bytes = Vec::with_capacity(genesis_header.encoded_len());
                genesis_header
                    .encode(&mut block_header_bytes)
                    .expect("encode block header failed");
                let block_hash = account.hash(block_header_bytes.as_slice());
                info!("current block hash: {}", hex(block_hash.as_slice()));
                Self::step_next(con, block_hash)?;
                Ok(())
            } else {
                Err(anyhow!(
                    "execute local genesis block failed, status code: {:?}",
                    status
                ))
            }
        } else {
            Err(anyhow!("commit genesis block failed!"))
        }
    }
}
