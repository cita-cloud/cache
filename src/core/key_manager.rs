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

use crate::cita_cloud::{controller::ControllerBehaviour, evm::EvmBehaviour};
use crate::common::constant::{
    controller, evm, local_executor, rough_internal, ADMIN_ACCOUNT, CITA_CLOUD_BLOCK_NUMBER,
    COMMITTED_TX, CONTRACT_KEY, CURRENT_BATCH_NUMBER, CURRENT_FAKE_BLOCK_HASH, EVICT_TO_ROUGH_TIME,
    EXPIRED_KEY_EVENT_AT_ALL_DB, HASH_TO_BLOCK_NUMBER, HASH_TO_TX, HASH_TYPE, KEY_PREFIX,
    LAZY_EVICT_TO_TIME, ONE_THOUSAND, PACKAGED_TX, PACK_UNCOMMITTED_TX, ROLLUP_WRITE_ENABLE,
    SET_TYPE, SYSTEM_CONFIG, TIME_TO_CLEAN_UP, UNCOMMITTED_TX, VALIDATE_TX_BUFFER,
    VALIDATOR_BATCH_NUMBER, VAL_TYPE, ZSET_TYPE,
};
use crate::common::util::{hex_without_0x, parse_hash, timestamp};
use crate::redis::{hexists, sadd, sismember, smove, ttl};
use crate::{
    delete, exists, get, hdel, hget, hset, incr_one, keys, psubscribe, smembers, srem, zadd,
    zrange_withscores, zrem, ArrayLike, Display, Hash, RECEIPT, TX,
};
use anyhow::{anyhow, Result};
use cita_cloud_proto::blockchain::{raw_transaction::Tx, Block, RawTransaction};

use crate::cita_cloud::controller::{SignerBehaviour, TransactionSenderBehaviour};
use crate::cita_cloud::executor::ExecutorBehaviour;
use crate::cita_cloud::wallet::MaybeLocked;
use crate::common::context::{BlockContext, LocalBehaviour};
use crate::common::package::Package;
use crate::rest_api::post::ToTx;
use msgpack_schema::deserialize;
use prost::Message;
use r2d2_redis::redis::{ControlFlow, FromRedisValue, ToRedisArgs};
use serde_json::{json, Value};
use std::future::Future;

fn uncommitted_tx_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, UNCOMMITTED_TX)
}

fn pack_uncommitted_tx_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, PACK_UNCOMMITTED_TX)
}

fn committed_tx_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, COMMITTED_TX)
}

pub fn validate_tx_buffer() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, VALIDATE_TX_BUFFER)
}

pub fn hash_to_tx() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, HASH_TO_TX)
}

fn hash_to_block_number() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, HASH_TO_BLOCK_NUMBER)
}

fn clean_up_key(time: u64) -> String {
    format!("{}:{}:{}:{}", KEY_PREFIX, SET_TYPE, TIME_TO_CLEAN_UP, time)
}

fn clean_up_prefix() -> String {
    format!("{}:{}:{}:", KEY_PREFIX, SET_TYPE, TIME_TO_CLEAN_UP)
}

fn packaged_tx() -> String {
    format!("{}:{}:{}:", KEY_PREFIX, SET_TYPE, PACKAGED_TX)
}

fn lazy_evict_to_time() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, LAZY_EVICT_TO_TIME)
}

fn evict_to_rough_time() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, EVICT_TO_ROUGH_TIME)
}

fn val_prefix() -> String {
    format!("{}:{}", KEY_PREFIX, VAL_TYPE)
}

pub fn current_batch_number() -> String {
    format!("{}:{}:{}", KEY_PREFIX, VAL_TYPE, CURRENT_BATCH_NUMBER)
}

pub fn validator_batch_number() -> String {
    format!("{}:{}:{}", KEY_PREFIX, VAL_TYPE, VALIDATOR_BATCH_NUMBER)
}

pub fn current_fake_block_hash() -> String {
    format!("{}:{}:{}", KEY_PREFIX, VAL_TYPE, CURRENT_FAKE_BLOCK_HASH)
}

pub fn rollup_write_enable() -> String {
    format!("{}:{}:{}", KEY_PREFIX, VAL_TYPE, ROLLUP_WRITE_ENABLE)
}
pub fn cita_cloud_block_number_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, VAL_TYPE, CITA_CLOUD_BLOCK_NUMBER)
}

pub fn system_config_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, VAL_TYPE, SYSTEM_CONFIG)
}

pub fn admin_account_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, VAL_TYPE, ADMIN_ACCOUNT)
}

fn clean_up_pattern() -> String {
    format!("{}*", clean_up_prefix())
}

fn current_clean_up_key() -> String {
    clean_up_key(current_rough_time())
}

fn current_rough_time() -> u64 {
    let current = timestamp();
    current - current % rough_internal()
}

pub fn contract_key(to: String, data: String, height: u64) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        val_prefix(),
        CONTRACT_KEY,
        to,
        data,
        height
    )
}

pub fn key(key_type: String, param: String) -> String {
    format!("{}:{}:{}", val_prefix(), key_type, param)
}

pub fn key_without_param(key_type: String) -> String {
    format!("{}:{}", val_prefix(), key_type)
}

fn contract_pattern(to: String) -> String {
    format!("{}:{}:{}*", val_prefix(), CONTRACT_KEY, to)
}

async fn check_if_timeout(tx_hash: String) -> Result<bool> {
    let current = controller().get_block_number(false).await?;
    let config = controller().get_system_config().await?;
    let valid_until_block = CacheManager::valid_until_block(tx_hash)?;
    Ok(valid_until_block <= current || valid_until_block > (current + config.block_limit as u64))
}

#[tonic::async_trait]
pub trait ExpiredBehavior {
    fn time_pair(timestamp: u64, internal: usize, rough_internal: u64) -> (u64, u64);

    fn rough_time(expire_time: u64, rough_internal: u64) -> u64;

    fn update_expire(key: String, seconds: usize) -> Result<()>;

    fn delete_expire(key: String, member: String) -> Result<()>;

    fn create_expire(key: String, seconds: usize) -> Result<()>;
}

#[tonic::async_trait]
pub trait ValBehavior {
    fn save_val(key: String, val: String, expire_time: usize) -> Result<String>;

    fn exist_val(key: String) -> Result<bool>;

    fn load_val(key: String, expire_time: usize) -> Result<String>;
}

#[tonic::async_trait]
pub trait TxBehavior {
    fn enqueue_tx(hash_str: String, tx: Vec<u8>, need_package: bool) -> Result<()>;
    fn commit_tx(tx_hash: String, score: u64) -> Result<()>;

    fn original_tx(tx_hash: String) -> Result<Vec<u8>>;

    fn valid_until_block(tx_hash: String) -> Result<u64>;

    fn save_valid_until_block(tx_hash: String, valid_until_block: u64) -> Result<u64>;

    fn uncommitted_txs(size: isize) -> Result<Vec<(String, u64)>>;

    fn pack_uncommitted_txs(size: isize) -> Result<Vec<(String, u64)>>;

    fn committed_txs(size: isize) -> Result<Vec<(String, u64)>>;

    fn clean_up_tx(tx_hash: String) -> Result<()>;
}

#[tonic::async_trait]
pub trait ContractBehavior {
    fn try_clean_contract_data(tx_hash: String) -> Result<()>;
    fn try_clean_contract(raw_tx: RawTransaction) -> Result<()>;
}

#[tonic::async_trait]
pub trait PackBehavior {
    async fn package(timing_batch: isize, expire_time: usize) -> Result<()>;
    fn is_packaged_tx(hash: String) -> Result<bool>;
    fn tag_tx(hash: String) -> Result<()>;
}

#[tonic::async_trait]
pub trait ValidatorBehavior {
    fn enqueue_to_buffer(hash_str: String, package_data: Vec<u8>, batch_number: u64) -> Result<()>;
    fn dequeue_smallest_from_buffer() -> Result<Vec<(String, u64)>>;
    fn clean(member: String) -> Result<()>;
    async fn poll(timing_batch: isize, expire_time: usize) -> Result<()>;
    async fn replay(timing_batch: isize, expire_time: usize) -> Result<()>;
}

#[tonic::async_trait]
pub trait CacheBehavior:
    ExpiredBehavior + ValBehavior + ContractBehavior + PackBehavior + ValidatorBehavior
{
    fn enqueue(
        hash_str: String,
        tx: Vec<u8>,
        valid_util_block: u64,
        need_package: bool,
    ) -> Result<()>;
    async fn load_or_query<F, T>(key: String, expire_time: usize, f: F) -> Result<Value>
    where
        T: Display,
        F: Send + Future<Output = Result<T>>;

    async fn load_or_query_obj<F, T>(
        key: String,
        expire_time: usize,
        f: F,
        is_obj: bool,
    ) -> Result<Value>
    where
        T: Display,
        F: Send + Future<Output = Result<T>>;

    fn save_tx_content(tx_hash: String, tx: String, expire_time: usize) -> Result<()>;

    fn save_receipt_content(tx_hash: String, tx: String, expire_time: usize) -> Result<()>;

    fn save_error(hash: String, err_str: String, expire_time: usize) -> Result<()>;

    fn clean_up_expired_by_key(expired_key: String) -> Result<String>;

    fn set_ex<T: Clone + Default + FromRedisValue + ToRedisArgs>(
        key: String,
        val: T,
        seconds: usize,
    ) -> Result<String>;

    fn expire(key: String, seconds: usize) -> Result<u64>;

    fn clean_up_expired(key: String, member: String) -> Result<()>;

    fn clean_up_packaged_txs(hash_list: Vec<String>) -> Result<()>;

    async fn commit(timing_batch: isize, expire_time: usize) -> Result<()>;

    async fn check(timing_batch: isize, expire_time: usize) -> Result<()>;

    async fn try_lazy_evict() -> Result<()>;

    async fn sub_evict_event() -> Result<()>;

    async fn set_up() -> Result<()>;
}

pub struct CacheManager;

#[tonic::async_trait]
impl ExpiredBehavior for CacheManager {
    fn time_pair(timestamp: u64, internal: usize, rough_internal: u64) -> (u64, u64) {
        let expire_time = timestamp + internal as u64 * ONE_THOUSAND;
        //key 之前 internal内过期的key
        let rough_time = Self::rough_time(expire_time, rough_internal);
        (expire_time, rough_time)
    }

    fn rough_time(expire_time: u64, rough_internal: u64) -> u64 {
        expire_time - expire_time % rough_internal + rough_internal
    }

    //CacheManager::set_up()会清理掉过期的key，若被清理create_expire
    fn update_expire(key: String, seconds: usize) -> Result<()> {
        if hexists(lazy_evict_to_time(), key.clone())? {
            let old_expire_time = hget(lazy_evict_to_time(), key.clone())?;
            let rough_internal = rough_internal();
            let old_rough_time = Self::rough_time(old_expire_time, rough_internal);

            let (expire_time, rough_time) = Self::time_pair(timestamp(), seconds, rough_internal);

            smove(
                clean_up_key(old_rough_time),
                clean_up_key(rough_time),
                key.clone(),
            )?;
            hset(lazy_evict_to_time(), key.clone(), expire_time)?;
            hset(evict_to_rough_time(), key, clean_up_key(rough_time))?;
            Ok(())
        } else {
            Self::create_expire(key, seconds)
        }
    }

    fn delete_expire(key: String, member: String) -> Result<()> {
        hdel(lazy_evict_to_time(), member.clone())?;
        hdel(evict_to_rough_time(), member.clone())?;
        srem(key, member)?;
        Ok(())
    }

    fn create_expire(key: String, seconds: usize) -> Result<()> {
        let (expire_time, rough_time) = Self::time_pair(timestamp(), seconds, rough_internal());
        sadd(clean_up_key(rough_time), key.clone())?;
        hset(lazy_evict_to_time(), key.clone(), expire_time)?;
        hset(evict_to_rough_time(), key, clean_up_key(rough_time))?;
        Ok(())
    }
}

#[tonic::async_trait]
impl TxBehavior for CacheManager {
    fn enqueue_tx(hash_str: String, tx: Vec<u8>, need_package: bool) -> Result<()> {
        let key = if need_package {
            pack_uncommitted_tx_key()
        } else {
            uncommitted_tx_key()
        };
        zadd(key, hash_str.clone(), timestamp())?;
        hset(hash_to_tx(), hash_str, tx)?;
        Ok(())
    }

    fn commit_tx(tx_hash: String, score: u64) -> Result<()> {
        zrem(uncommitted_tx_key(), tx_hash.clone())?;
        zadd(committed_tx_key(), tx_hash, score)?;
        Ok(())
    }

    fn original_tx(tx_hash: String) -> Result<Vec<u8>> {
        let tx = hget::<Vec<u8>>(hash_to_tx(), tx_hash)?;
        Ok(tx)
    }

    fn valid_until_block(tx_hash: String) -> Result<u64> {
        let valid_until_block = hget::<u64>(hash_to_block_number(), tx_hash)?;
        Ok(valid_until_block)
    }

    fn save_valid_until_block(tx_hash: String, valid_until_block: u64) -> Result<u64> {
        let result = hset(hash_to_block_number(), tx_hash, valid_until_block)?;
        Ok(result)
    }

    fn uncommitted_txs(size: isize) -> Result<Vec<(String, u64)>> {
        let result = zrange_withscores::<String>(uncommitted_tx_key(), 0, size)?;
        Ok(result)
    }

    fn pack_uncommitted_txs(size: isize) -> Result<Vec<(String, u64)>> {
        let result = zrange_withscores::<String>(pack_uncommitted_tx_key(), 0, size)?;
        Ok(result)
    }

    fn committed_txs(size: isize) -> Result<Vec<(String, u64)>> {
        let result = zrange_withscores::<String>(committed_tx_key(), 0, size)?;
        Ok(result)
    }

    fn clean_up_tx(tx_hash: String) -> Result<()> {
        zrem(pack_uncommitted_tx_key(), tx_hash.clone())?;
        zrem(committed_tx_key(), tx_hash.clone())?;
        zrem(uncommitted_tx_key(), tx_hash.clone())?;
        hdel(hash_to_tx(), tx_hash.clone())?;
        hdel(hash_to_block_number(), tx_hash.clone())?;
        srem(packaged_tx(), tx_hash)?;
        Ok(())
    }
}

#[tonic::async_trait]
impl ValBehavior for CacheManager {
    fn save_val(key: String, val: String, expire_time: usize) -> Result<String> {
        Self::set_ex(key, val, expire_time)
    }

    fn exist_val(key: String) -> Result<bool> {
        match ttl(key) {
            Ok(time) => Ok(time > 0),
            Err(_) => Ok(false),
        }
    }

    fn load_val(key: String, expire_time: usize) -> Result<String> {
        Self::expire(key.clone(), expire_time)?;
        let val = get(key)?;
        Ok(val)
    }
}

#[tonic::async_trait]
impl ContractBehavior for CacheManager {
    fn try_clean_contract_data(tx_hash: String) -> Result<()> {
        let tx = Self::original_tx(tx_hash)?;
        let decoded: RawTransaction = Message::decode(tx.as_slice())?;
        Self::try_clean_contract(decoded)?;
        Ok(())
    }

    fn try_clean_contract(raw_tx: RawTransaction) -> Result<()> {
        if let Some(Tx::NormalTx(normal_tx)) = raw_tx.tx {
            if let Some(transaction) = normal_tx.transaction {
                let addr = hex_without_0x(&transaction.to);
                if let Ok(keys) = keys::<String>(contract_pattern(addr)) {
                    if !keys.is_empty() {
                        delete(keys)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[tonic::async_trait]
impl PackBehavior for CacheManager {
    async fn package(timing_batch: isize, _expire_time: usize) -> Result<()> {
        let members = CacheManager::pack_uncommitted_txs(timing_batch)?;
        if members.is_empty() {
            return Ok(());
        }
        let mut tx_list = Vec::new();
        let mut hash_list = Vec::new();
        for (tx_hash, _score) in members {
            let tx = Self::original_tx(tx_hash.clone())?;
            let decoded: RawTransaction = Message::decode(tx.as_slice())?;
            tx_list.push(decoded);
            hash_list.push(tx_hash);
        }
        let batch_number = BlockContext::get_batch_number().await?;
        let maybe: MaybeLocked = BlockContext::current_account()?;
        let account = maybe.unlocked()?;
        let proposer = account.address().to_vec();
        let block = BlockContext::fake_block(proposer, tx_list.clone()).await?;

        if let Ok(res) = local_executor().exec(block.clone()).await {
            if let Some(status) = res.status {
                if status.code == 0 {
                    for raw_tx in tx_list {
                        Self::try_clean_contract(raw_tx)?;
                    }

                    let packaged_tx_obj = Package::new(batch_number, block.clone())
                        .to_packaged_tx(*account.address())?;
                    let hash = controller()
                        .send_raw_tx(account, packaged_tx_obj.to(account, evm()).await?, false)
                        .await?;
                    Self::clean_up_packaged_txs(hash_list)?;
                    Self::tag_tx(hex_without_0x(hash.as_slice()))?;
                    info!("package batch: {}.", batch_number);
                }
            }
        }
        Ok(())
    }

    fn is_packaged_tx(hash: String) -> Result<bool> {
        Ok(sismember(packaged_tx(), hash)?)
    }

    fn tag_tx(hash: String) -> Result<()> {
        sadd(packaged_tx(), hash)?;
        Ok(())
    }
}

#[tonic::async_trait]
impl ValidatorBehavior for CacheManager {
    fn enqueue_to_buffer(hash_str: String, package_data: Vec<u8>, batch_number: u64) -> Result<()> {
        info!(
            "enqueue tx to validator buffer, hash: {}, batch number: {}",
            hash_str, batch_number
        );
        zadd(validate_tx_buffer(), hash_str.clone(), batch_number)?;
        hset(hash_to_tx(), hash_str, package_data)?;
        Ok(())
    }

    fn dequeue_smallest_from_buffer() -> Result<Vec<(String, u64)>> {
        Ok(zrange_withscores::<String>(validate_tx_buffer(), 0, 0)?)
    }

    fn clean(member: String) -> Result<()> {
        zrem(validate_tx_buffer(), member.clone())?;
        hdel(hash_to_tx(), member)?;
        Ok(())
    }

    async fn poll(_timing_batch: isize, _expire_time: usize) -> Result<()> {
        let account = BlockContext::current_account()?;
        let cita_height = BlockContext::current_cita_height()?;
        let key = validator_batch_number();
        let validator_current_height = if exists(key.clone())? {
            get(key.clone())?
        } else {
            incr_one(key.clone())?
        };
        if validator_current_height >= cita_height {
            return Ok(());
        }
        info!("validate cita cloud height: {}", validator_current_height);
        let compact_block = controller()
            .get_block_by_number(validator_current_height)
            .await?;
        let tx_hashs: Vec<Vec<u8>> = compact_block
            .body
            .expect("get compact body failed!")
            .tx_hashes;
        for hash in tx_hashs {
            info!(
                "validate cita cloud block [{}], has package txs",
                validator_current_height
            );
            let raw = controller()
                .get_tx(Hash::try_from_slice(hash.as_slice())?)
                .await?;
            if let Some(Tx::NormalTx(normal_tx)) = raw.tx {
                let sender: Vec<u8> = normal_tx.witness.expect("get witness failed!").sender;
                if sender == account.address().to_vec() {
                    let package_data = normal_tx.transaction.expect("get transaction failed!").data;
                    let decoded_package = deserialize::<Package>(package_data.clone().as_slice())?;
                    let batch_number = decoded_package.batch_number;
                    info!("poll batch: {}!", batch_number);
                    let hash_str = hex_without_0x(normal_tx.transaction_hash.as_slice());
                    Self::enqueue_to_buffer(hash_str, package_data, batch_number)?;
                }
            }
        }
        incr_one(key)?;
        Ok(())
    }

    async fn replay(_timing_batch: isize, _expire_time: usize) -> Result<()> {
        for (member, batch_number) in Self::dequeue_smallest_from_buffer()? {
            if batch_number == BlockContext::get_batch_number().await? {
                info!("replay batch: {}!", batch_number);
                let maybe = BlockContext::current_account()?;
                let account = maybe.unlocked()?;

                let raw = Self::original_tx(member.clone())?;
                let decoded_package = deserialize::<Package>(raw.as_slice())?;
                let block: Block = Message::decode(decoded_package.block.as_slice())?;

                let mut header = block.header.expect("get block header failed");
                header.prevhash = BlockContext::get_fake_block_hash().await?;
                if let Ok(res) = local_executor()
                    .exec(Block {
                        version: 0,
                        header: Some(header.clone()),
                        body: block.body.clone(),
                        proof: Vec::new(),
                        state_root: Vec::new(),
                    })
                    .await
                {
                    if let Some(status) = res.status {
                        if status.code == 0 {
                            for raw_tx in block.body.expect("get block body failed").body {
                                Self::try_clean_contract(raw_tx)?;
                            }
                            let mut block_header_bytes = Vec::with_capacity(header.encoded_len());
                            header
                                .encode(&mut block_header_bytes)
                                .expect("encode block header failed");
                            let block_hash = account.hash(block_header_bytes.as_slice());
                            BlockContext::step_next(block_hash)?;
                            Self::clean(member)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
#[tonic::async_trait]
impl CacheBehavior for CacheManager {
    fn enqueue(
        hash_str: String,
        tx: Vec<u8>,
        valid_util_block: u64,
        need_package: bool,
    ) -> Result<()> {
        Self::save_valid_until_block(hash_str.clone(), valid_util_block)?;
        Self::enqueue_tx(hash_str, tx, need_package)
    }

    async fn load_or_query<F, T>(key: String, expire_time: usize, f: F) -> Result<Value>
    where
        T: Display,
        F: Send + Future<Output = Result<T>>,
    {
        Self::load_or_query_obj(key, expire_time, f, false).await
    }

    async fn load_or_query_obj<F, T>(
        key: String,
        expire_time: usize,
        f: F,
        is_obj: bool,
    ) -> Result<Value>
    where
        T: Display,
        F: Send + Future<Output = Result<T>>,
    {
        if Self::exist_val(key.clone())? {
            let result = Self::load_val(key, expire_time)?;
            let content = result.as_str();
            match serde_json::from_str(content) {
                Ok(json) => Ok(json),
                Err(e) => {
                    if is_obj {
                        Err(anyhow!(e))
                    } else {
                        Ok(json!(content))
                    }
                }
            }
        } else {
            let val: T = f.await?;
            Self::save_val(key, val.display(), expire_time)?;
            Ok(val.to_json())
        }
    }

    fn save_tx_content(tx_hash: String, content: String, expire_time: usize) -> Result<()> {
        Self::set_ex(key(TX.to_string(), tx_hash), content, expire_time)?;
        Ok(())
    }

    fn save_receipt_content(tx_hash: String, content: String, expire_time: usize) -> Result<()> {
        Self::set_ex(key(RECEIPT.to_string(), tx_hash), content, expire_time)?;
        Ok(())
    }

    fn save_error(hash: String, err_str: String, expire_time: usize) -> Result<()> {
        Self::save_tx_content(hash.clone(), err_str.clone(), expire_time)?;
        Self::save_receipt_content(hash, err_str, expire_time)
    }

    fn clean_up_expired_by_key(expired_key: String) -> Result<String> {
        if hexists(evict_to_rough_time(), expired_key.clone())? {
            let key = hget::<String>(evict_to_rough_time(), expired_key.clone())?;
            Self::clean_up_expired(key, expired_key.clone())?;
        }
        Ok(expired_key)
    }

    fn set_ex<T: Clone + Default + FromRedisValue + ToRedisArgs>(
        key: String,
        val: T,
        seconds: usize,
    ) -> Result<String> {
        if exists(key.clone())? {
            Self::update_expire(key.clone(), seconds)?;
        } else {
            Self::create_expire(key.clone(), seconds)?;
        }
        let result = crate::redis::set_ex(key, val, seconds)?;
        Ok(result)
    }

    fn expire(key: String, seconds: usize) -> Result<u64> {
        Self::update_expire(key.clone(), seconds)?;
        let result = crate::redis::expire(key, seconds)?;
        Ok(result)
    }

    fn clean_up_expired(key: String, member: String) -> Result<()> {
        Self::delete_expire(key, member)
    }

    fn clean_up_packaged_txs(hash_list: Vec<String>) -> Result<()> {
        for hash in hash_list {
            zrem(pack_uncommitted_tx_key(), hash)?;
        }
        Ok(())
    }

    async fn commit(timing_batch: isize, expire_time: usize) -> Result<()> {
        let members = CacheManager::uncommitted_txs(timing_batch)?;
        for (tx_hash, score) in members {
            let tx = Self::original_tx(tx_hash.clone())?;
            let decoded: RawTransaction = Message::decode(tx.as_slice())?;
            match controller().send_raw(decoded.clone()).await {
                Ok(data) => {
                    Self::commit_tx(tx_hash.clone(), score)?;
                    let hash_str = hex_without_0x(&data);
                    info!("commit tx success, hash: {}", hash_str);
                }
                Err(e) => {
                    let empty = Vec::new();
                    let hash = if let Some(Tx::NormalTx(ref normal_tx)) = decoded.tx {
                        &normal_tx.transaction_hash
                    } else {
                        empty.as_slice()
                    };
                    let hash = hex_without_0x(hash).to_string();
                    Self::save_error(hash.clone(), format!("{}", e), expire_time * 5)?;
                    Self::clean_up_tx(hash.clone())?;
                    warn!("commit tx fail, hash: {}", hash);
                }
            }
        }
        Ok(())
    }

    async fn check(timing_batch: isize, expire_time: usize) -> Result<()> {
        let members = CacheManager::committed_txs(timing_batch)?;
        for (tx_hash, _) in members {
            let hash = parse_hash(tx_hash.clone().as_str())?;
            let (receipt, expire_time, is_ok) = match evm().get_receipt(hash).await {
                Ok(receipt) => {
                    info!("get tx receipt success, hash: {}", tx_hash.clone());
                    (receipt.display(), expire_time, true)
                }
                Err(e) => {
                    info!("retry -> get receipt, hash: {}", tx_hash.clone());
                    (format!("{}", e), expire_time * 5, false)
                }
            };
            CacheManager::save_receipt_content(tx_hash.clone(), receipt, expire_time)?;
            if is_ok {
                if Self::is_packaged_tx(tx_hash.clone())? {
                    let raw_tx = Self::original_tx(tx_hash.clone())?;
                    let raw_tx: RawTransaction = Message::decode(raw_tx.as_slice())?;
                    if let Some(Tx::NormalTx(normal_tx)) = raw_tx.tx {
                        if let Some(transaction) = normal_tx.transaction {
                            let package_data = deserialize::<Package>(transaction.data.as_slice())?;
                            let block: Block = Message::decode(package_data.block.as_slice())?;

                            let header = block.header.expect("get block header failed");
                            let mut block_header_bytes = Vec::with_capacity(header.encoded_len());
                            header
                                .encode(&mut block_header_bytes)
                                .expect("encode block header failed");
                            let maybe: MaybeLocked = BlockContext::current_account()?;
                            let account = maybe.unlocked()?;
                            let block_hash = account.hash(block_header_bytes.as_slice());
                            BlockContext::step_next(block_hash)?;
                        }
                    }
                }
                let (tx, expire_time) = match controller().get_tx(hash).await {
                    Ok(tx) => {
                        info!("get tx success, hash: {}", tx_hash.clone());
                        (tx.display(), expire_time)
                    }
                    Err(e) => {
                        info!("retry -> get tx, hash: {}", tx_hash.clone());
                        (format!("{}", e), expire_time * 5)
                    }
                };

                CacheManager::save_tx_content(tx_hash.clone(), tx, expire_time)?;
                CacheManager::try_clean_contract_data(tx_hash.clone())?;
                CacheManager::clean_up_tx(tx_hash.clone())?;

                continue;
            }
            if check_if_timeout(tx_hash.clone()).await? {
                if Self::is_packaged_tx(tx_hash.clone())? {
                    let tx = Self::original_tx(tx_hash.clone())?;
                    let decoded: RawTransaction = Message::decode(tx.as_slice())?;
                    if let Some(Tx::NormalTx(normal_tx)) = decoded.tx {
                        let package_data =
                            normal_tx.transaction.expect("get transaction failed!").data;
                        let decoded_package = deserialize::<Package>(package_data.as_slice())?;
                        let maybe: MaybeLocked = BlockContext::current_account()?;
                        let account = maybe.unlocked()?;
                        let new_package = decoded_package.to_packaged_tx(*account.address())?;
                        controller()
                            .send_raw_tx(account, new_package.to(account, evm()).await?, false)
                            .await?;
                        warn!("repackage batch: {}.", decoded_package.batch_number);
                    }
                } else {
                    CacheManager::save_error(
                        tx_hash.clone(),
                        "timeout".to_string(),
                        expire_time * 5,
                    )?;
                    CacheManager::clean_up_tx(tx_hash.clone())?;
                    warn!("retry -> get receipt, timeout hash: {}", tx_hash);
                }
            }
        }
        Ok(())
    }

    async fn try_lazy_evict() -> Result<()> {
        let key = current_clean_up_key();
        if exists(key.clone())? {
            for member in smembers::<String>(key.clone())? {
                if get::<String>(member.clone()).is_err() {
                    info!("lazy evict key: {}", member);
                }
                Self::clean_up_expired(key.clone(), member.clone())?;
            }
        }
        Ok(())
    }

    async fn sub_evict_event() -> Result<()> {
        psubscribe(EXPIRED_KEY_EVENT_AT_ALL_DB.to_string(), |msg| {
            let expired_key = match msg.get_payload::<String>() {
                Ok(key) => {
                    if key.starts_with(&val_prefix()) {
                        key
                    } else {
                        return ControlFlow::Continue;
                    }
                }
                Err(e) => {
                    warn!("subscribe msg has none payload: {}", e);
                    return ControlFlow::Continue;
                }
            };
            match CacheManager::clean_up_expired_by_key(expired_key) {
                Ok(expired_key) => info!("evict expired key: {}", expired_key),
                Err(e) => warn!("evict expired failed: {}", e),
            }
            ControlFlow::Continue
        })?;
        Ok(())
    }

    async fn set_up() -> Result<()> {
        let current = current_rough_time();
        for key in keys::<String>(clean_up_pattern())? {
            let rough_time_str: &str = &key[clean_up_prefix().len()..];
            if let Ok(rough_time) = rough_time_str.parse::<u64>() {
                if rough_time < current {
                    if let Ok(members) = smembers::<String>(key) {
                        for member in members {
                            if Self::clean_up_expired_by_key(member.clone()).is_ok() {
                                info!("set up -> clean up expired key: {} success", member);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
