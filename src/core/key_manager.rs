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

use crate::common::constant::*;
use crate::common::util::{hex_without_0x, parse_hash, timestamp};
use crate::redis::{hexists, sadd, set, sismember, smove, ttl, xadd, Connection};
use crate::{
    delete, exists, get, hdel, hget, hset, incr_one, keys, psubscribe, smembers, srem, zadd,
    zrange_withscores, zrem, ArrayLike, DasAdaptor, Display, Hash, RECEIPT, TX,
};
use anyhow::Result;
use cita_cloud_proto::blockchain::{raw_transaction::Tx, Block, CompactBlock, RawTransaction};
use std::cmp::Ordering;

use crate::cita_cloud::controller::SignerBehaviour;
use crate::cita_cloud::executor::ExecutorBehaviour;
use crate::cita_cloud::wallet::MaybeLocked;
use crate::common::context::{BlockContext, LocalBehaviour};
use crate::common::package::Package;
use crate::rest_api::post::ToTx;
use msgpack_schema::{deserialize, serialize};
use prost::Message;
use r2d2_redis::redis::{Commands, ControlFlow, FromRedisValue, ToRedisArgs, Value as RedisValue};
use serde_json::Value;

use crate::adaptor::layer1_adaptor::Layer1Adaptor;
use crate::common::cache_log::CtxMap;
use crate::core::schedule_task::get_con;
use crate::core::schedule_task::Expire;
use cita_cloud_proto::blockchain::Transaction as CloudNormalTransaction;
use cita_cloud_proto::common::HashResponse;
use opentelemetry::global;
use r2d2_redis::redis::streams::{StreamReadOptions, StreamReadReply};
use std::future::Future;
use tracing::instrument;
use tracing::{info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

fn uncommitted_tx_key() -> String {
    format!("{KEY_PREFIX}:{ZSET_TYPE}:{UNCOMMITTED_TX}")
}

fn pack_uncommitted_tx_key() -> String {
    format!("{KEY_PREFIX}:{ZSET_TYPE}:{PACK_UNCOMMITTED_TX}")
}

fn committed_tx_key() -> String {
    format!("{KEY_PREFIX}:{ZSET_TYPE}:{COMMITTED_TX}")
}

pub fn validate_tx_buffer() -> String {
    format!("{KEY_PREFIX}:{ZSET_TYPE}:{VALIDATE_TX_BUFFER}")
}

pub fn hash_to_tx() -> String {
    format!("{KEY_PREFIX}:{HASH_TYPE}:{HASH_TO_TX}")
}
pub fn save_block_key(hash: String) -> String {
    format!("{KEY_PREFIX}:{HASH_TYPE}:{SAVE_BLOCK}:{hash}")
}

pub fn hash_to_trace_ctx() -> String {
    format!("{KEY_PREFIX}:{HASH_TYPE}:{HASH_TO_TRACE_CTX}")
}

fn hash_to_block_number() -> String {
    format!("{KEY_PREFIX}:{HASH_TYPE}:{HASH_TO_BLOCK_NUMBER}")
}

fn clean_up_key(time: u64) -> String {
    format!("{KEY_PREFIX}:{SET_TYPE}:{TIME_TO_CLEAN_UP}:{time}")
}

fn clean_up_prefix() -> String {
    format!("{KEY_PREFIX}:{SET_TYPE}:{TIME_TO_CLEAN_UP}:")
}

fn packaged_tx() -> String {
    format!("{KEY_PREFIX}:{SET_TYPE}:{PACKAGED_TX}:")
}

fn block_to_tx(block_hash: String) -> String {
    format!("{KEY_PREFIX}:{SET_TYPE}:{BLOCK_TO_TX}:{block_hash}")
}

fn lazy_evict_to_time() -> String {
    format!("{KEY_PREFIX}:{HASH_TYPE}:{LAZY_EVICT_TO_TIME}")
}

fn evict_to_rough_time() -> String {
    format!("{KEY_PREFIX}:{HASH_TYPE}:{EVICT_TO_ROUGH_TIME}")
}

fn val_prefix() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}")
}

pub fn current_batch_number() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{CURRENT_BATCH_NUMBER}")
}

pub fn validator_batch_number() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{VALIDATOR_BATCH_NUMBER}")
}

pub fn current_fake_block_hash() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{CURRENT_FAKE_BLOCK_HASH}")
}

pub fn batch_to_state_root() -> String {
    format!("{KEY_PREFIX}:{HASH_TYPE}:{BATCH_NUMBER_TO_STATE_ROOT}")
}

pub fn rollup_write_enable() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{ROLLUP_WRITE_ENABLE}")
}
pub fn cita_cloud_block_number_key() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{CITA_CLOUD_BLOCK_NUMBER}")
}

pub fn system_config_key() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{SYSTEM_CONFIG}")
}

pub fn admin_account_key() -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{ADMIN_ACCOUNT}")
}

pub fn stream_id_key(name: String) -> String {
    format!("{KEY_PREFIX}:{VAL_TYPE}:{STREAM_ID}:{name}")
}

pub fn stream_key(name: String) -> String {
    format!("{KEY_PREFIX}:{STREAM_TYPE}:{name}")
}

pub fn stream_key_suffix(mut key: String) -> String {
    key.split_off(KEY_PREFIX.len() + 1 + STREAM_TYPE.len() + 1)
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

async fn check_if_timeout(con: &mut Connection, tx_hash: String) -> Result<bool> {
    let current = BlockContext::current_cita_height(con)?;
    let config = BlockContext::system_config(con)?;
    let valid_until_block = CacheOperator::valid_until_block(con, tx_hash)?;
    Ok(valid_until_block <= current || valid_until_block > (current + config.block_limit as u64))
}

#[tonic::async_trait]
pub trait ExpiredBehavior {
    fn time_pair(timestamp: u64, internal: usize, rough_internal: u64) -> (u64, u64);

    fn rough_time(expire_time: u64, rough_internal: u64) -> u64;

    fn update_expire(con: &mut Connection, key: String, seconds: usize) -> Result<()>;

    fn delete_expire(con: &mut Connection, key: String, member: String) -> Result<()>;

    fn create_expire(con: &mut Connection, key: String, seconds: usize) -> Result<()>;
}

#[tonic::async_trait]
pub trait ValBehavior {
    fn save_val<T: Clone + Default + FromRedisValue + ToRedisArgs>(
        con: &mut Connection,
        key: String,
        val: T,
        expire_time: usize,
    ) -> Result<String>;

    fn exist_val(con: &mut Connection, key: String) -> Result<bool>;

    fn load_val<T: Clone + Default + FromRedisValue + ToRedisArgs>(
        con: &mut Connection,
        key: String,
        expire_time: usize,
    ) -> Result<T>;
}

#[tonic::async_trait]
pub trait TxBehavior {
    fn enqueue_tx(con: &mut Connection, hash_str: String, tx: Vec<u8>) -> Result<()>;
    fn commit_tx(con: &mut Connection, tx_hash: String, score: u64) -> Result<()>;

    fn original_tx(con: &mut Connection, tx_hash: String) -> Result<Vec<u8>>;

    fn valid_until_block(con: &mut Connection, tx_hash: String) -> Result<u64>;

    fn save_valid_until_block(
        con: &mut Connection,
        tx_hash: String,
        valid_until_block: u64,
    ) -> Result<u64>;

    fn uncommitted_txs(con: &mut Connection, size: isize) -> Result<Vec<(String, u64)>>;

    fn pack_uncommitted_txs(con: &mut Connection, size: isize) -> Result<Vec<(String, u64)>>;

    fn committed_txs(con: &mut Connection, size: isize) -> Result<Vec<(String, u64)>>;

    fn clean_up_tx(con: &mut Connection, tx_hash: String) -> Result<()>;
}

#[tonic::async_trait]
pub trait ContractBehavior {
    fn try_clean_contract_data(con: &mut Connection, tx_hash: String) -> Result<()>;
    fn try_clean_contract(con: &mut Connection, raw_tx: RawTransaction) -> Result<()>;
}

pub struct CacheOperator;

#[tonic::async_trait]
impl ValBehavior for CacheOperator {
    fn save_val<T: Clone + Default + FromRedisValue + ToRedisArgs>(
        con: &mut Connection,
        key: String,
        val: T,
        expire_time: usize,
    ) -> Result<String> {
        let config = config();
        if config.enable_evict {
            if exists(con, key.clone())? {
                Self::update_expire(con, key.clone(), expire_time)?;
            } else {
                Self::create_expire(con, key.clone(), expire_time)?;
            }
        }
        let result = crate::redis::set_ex(con, key, val, expire_time)?;
        Ok(result)
    }

    // #[instrument(skip_all)]
    fn exist_val(con: &mut Connection, key: String) -> Result<bool> {
        match ttl(con, key) {
            Ok(time) => Ok(time > 0),
            Err(_) => Ok(false),
        }
    }

    // #[instrument(skip_all)]
    fn load_val<T: Clone + Default + FromRedisValue + ToRedisArgs>(
        con: &mut Connection,
        key: String,
        expire_time: usize,
    ) -> Result<T> {
        let config = config();
        if config.enable_evict {
            let data = serialize(Expire::new(key.clone(), expire_time as u64));
            let list = vec![("data".to_string(), data.as_slice())];
            xadd::<&[u8]>(
                con,
                stream_key(EXPIRE.to_string()),
                "*".to_string(),
                list.as_slice(),
            )?;
        }
        crate::redis::expire(con, key.clone(), expire_time)?;
        Ok(get(con, key)?)
    }
}

#[tonic::async_trait]
impl TxBehavior for CacheOperator {
    fn enqueue_tx(con: &mut Connection, hash_str: String, tx: Vec<u8>) -> Result<()> {
        zadd(con, uncommitted_tx_key(), hash_str.clone(), timestamp())?;
        hset(con, hash_to_tx(), hash_str, tx)?;
        Ok(())
    }

    fn commit_tx(con: &mut Connection, tx_hash: String, score: u64) -> Result<()> {
        zrem(con, uncommitted_tx_key(), tx_hash.clone())?;
        zadd(con, committed_tx_key(), tx_hash, score)?;
        Ok(())
    }

    fn original_tx(con: &mut Connection, tx_hash: String) -> Result<Vec<u8>> {
        let tx = hget::<String, Vec<u8>>(con, hash_to_tx(), tx_hash)?;
        Ok(tx)
    }

    fn valid_until_block(con: &mut Connection, tx_hash: String) -> Result<u64> {
        let valid_until_block = hget::<String, u64>(con, hash_to_block_number(), tx_hash)?;
        Ok(valid_until_block)
    }

    fn save_valid_until_block(
        con: &mut Connection,
        tx_hash: String,
        valid_until_block: u64,
    ) -> Result<u64> {
        let result = hset(con, hash_to_block_number(), tx_hash, valid_until_block)?;
        Ok(result)
    }

    fn uncommitted_txs(con: &mut Connection, size: isize) -> Result<Vec<(String, u64)>> {
        let result = zrange_withscores::<String>(con, uncommitted_tx_key(), 0, size)?;
        Ok(result)
    }

    fn pack_uncommitted_txs(con: &mut Connection, size: isize) -> Result<Vec<(String, u64)>> {
        let result = zrange_withscores::<String>(con, pack_uncommitted_tx_key(), 0, size)?;
        Ok(result)
    }

    fn committed_txs(con: &mut Connection, size: isize) -> Result<Vec<(String, u64)>> {
        let result = zrange_withscores::<String>(con, committed_tx_key(), 0, size)?;
        Ok(result)
    }

    fn clean_up_tx(con: &mut Connection, tx_hash: String) -> Result<()> {
        zrem(con, pack_uncommitted_tx_key(), tx_hash.clone())?;
        zrem(con, committed_tx_key(), tx_hash.clone())?;
        zrem(con, uncommitted_tx_key(), tx_hash.clone())?;
        hdel(con, hash_to_tx(), tx_hash.clone())?;
        hdel(con, hash_to_block_number(), tx_hash.clone())?;
        srem(con, packaged_tx(), tx_hash)?;
        Ok(())
    }
}

#[tonic::async_trait]
impl ExpiredBehavior for CacheOperator {
    fn time_pair(timestamp: u64, internal: usize, rough_internal: u64) -> (u64, u64) {
        let expire_time = timestamp + internal as u64 * ONE_THOUSAND;
        //key 之前 internal内过期的key
        let rough_time = Self::rough_time(expire_time, rough_internal);
        (expire_time, rough_time)
    }

    fn rough_time(expire_time: u64, rough_internal: u64) -> u64 {
        expire_time - expire_time % rough_internal + rough_internal
    }

    //set_up()会清理掉过期的key，若被清理create_expire
    fn update_expire(con: &mut Connection, key: String, seconds: usize) -> Result<()> {
        if hexists(con, lazy_evict_to_time(), key.clone())? {
            let old_expire_time = hget::<String, u64>(con, lazy_evict_to_time(), key.clone())?;
            let rough_internal = rough_internal();
            let old_rough_time = Self::rough_time(old_expire_time, rough_internal);

            let (expire_time, rough_time) = Self::time_pair(timestamp(), seconds, rough_internal);

            smove(
                con,
                clean_up_key(old_rough_time),
                clean_up_key(rough_time),
                key.clone(),
            )?;
            hset(con, lazy_evict_to_time(), key.clone(), expire_time)?;
            hset(con, evict_to_rough_time(), key, clean_up_key(rough_time))?;
            Ok(())
        } else {
            Self::create_expire(con, key, seconds)
        }
    }

    fn delete_expire(con: &mut Connection, key: String, member: String) -> Result<()> {
        hdel(con, lazy_evict_to_time(), member.clone())?;
        hdel(con, evict_to_rough_time(), member.clone())?;
        srem(con, key, member)?;
        Ok(())
    }

    fn create_expire(con: &mut Connection, key: String, seconds: usize) -> Result<()> {
        let (expire_time, rough_time) = Self::time_pair(timestamp(), seconds, rough_internal());
        sadd(con, clean_up_key(rough_time), key.clone())?;
        hset(con, lazy_evict_to_time(), key.clone(), expire_time)?;
        hset(con, evict_to_rough_time(), key, clean_up_key(rough_time))?;
        Ok(())
    }
}

#[instrument(skip_all)]
fn on_local_execute(ctx: CtxMap) {
    let parent_cx = global::get_text_map_propagator(|propagator| propagator.extract(&ctx.0));
    tracing::Span::current().set_parent(parent_cx);
}

#[instrument(skip_all)]
fn on_save_to_chain(ctx: CtxMap) {
    let parent_cx = global::get_text_map_propagator(|propagator| propagator.extract(&ctx.0));
    tracing::Span::current().set_parent(parent_cx);
}

#[tonic::async_trait]
impl ContractBehavior for CacheOperator {
    fn try_clean_contract_data(con: &mut Connection, tx_hash: String) -> Result<()> {
        let tx = Self::original_tx(con, tx_hash)?;
        let decoded: RawTransaction = Message::decode(tx.as_slice())?;
        Self::try_clean_contract(con, decoded)?;
        Ok(())
    }

    fn try_clean_contract(con: &mut Connection, raw_tx: RawTransaction) -> Result<()> {
        if let Some(Tx::NormalTx(normal_tx)) = raw_tx.tx {
            if let Some(transaction) = normal_tx.transaction {
                let addr = hex_without_0x(&transaction.to);
                if let Ok(keys) = keys::<String>(con, contract_pattern(addr)) {
                    if !keys.is_empty() {
                        delete(con, keys)?;
                    }
                }
            }
        }
        Ok(())
    }
}
#[tonic::async_trait]
pub trait PackBehavior {
    fn is_packaged_tx(con: &mut Connection, hash: String) -> Result<bool>;
    fn tag_tx(con: &mut Connection, hash: String) -> Result<()>;
}

#[tonic::async_trait]
pub trait CacheBehavior {
    #[instrument(skip_all)]
    fn save_trace_ctx(con: &mut Connection, hash_str: String, ctx: CtxMap) -> Result<()> {
        let ctx_bytes = serde_json::to_vec(&ctx)?;
        hset(con, hash_to_trace_ctx(), hash_str, ctx_bytes)?;
        Ok(())
    }

    fn get_trace_ctx(con: &mut Connection, hash_str: String) -> Result<CtxMap> {
        let ctx_bytes = hget::<String, Vec<u8>>(con, hash_to_trace_ctx(), hash_str)?;
        Ok(serde_json::from_slice(&ctx_bytes)?)
    }

    fn save_block_tx(con: &mut Connection, hash_str: String, tx_hash_str: String) -> Result<()> {
        sadd(con, block_to_tx(hash_str), tx_hash_str)?;
        Ok(())
    }

    fn get_block_txs(con: &mut Connection, hash_str: String) -> Result<Vec<String>> {
        Ok(smembers(con, block_to_tx(hash_str))?)
    }

    // #[instrument(skip_all)]
    async fn load_or_query_array_like<F, T>(
        con: &mut Connection,
        key: String,
        expire_time: usize,
        f: F,
    ) -> Result<Value>
    where
        T: Display + ArrayLike,
        F: Send + Future<Output = Result<T>>,
    {
        if CacheOperator::exist_val(con, key.clone())? {
            let result: Vec<u8> = CacheOperator::load_val(con, key, expire_time)?;
            let data: T = T::try_from_slice(result.as_slice())?;
            Ok(data.to_json())
        } else {
            let val: T = f.await?;
            CacheOperator::save_val(con, key, val.to_vec(), expire_time)?;
            Ok(val.to_json())
        }
    }

    // #[instrument(skip_all)]
    async fn load_or_query_proto<F, T>(
        con: &mut Connection,
        key: String,
        expire_time: usize,
        f: F,
    ) -> Result<Value>
    where
        T: Display + prost::Message + Default,
        F: Send + Future<Output = Result<T>>,
    {
        if CacheOperator::exist_val(con, key.clone())? {
            let result: Vec<u8> = CacheOperator::load_val(con, key, expire_time)?;
            match Message::decode(result.as_slice()) {
                Ok(data) => {
                    let val: T = data;
                    Ok(val.to_json())
                }
                Err(_e) => Ok(Value::String(String::from_utf8(result)?)),
            }
        } else {
            let val: T = f.await?;
            let mut val_bytes = Vec::with_capacity(val.encoded_len());
            val.encode(&mut val_bytes)
                .expect("encode system config failed");
            CacheOperator::save_val(con, key, val_bytes, expire_time)?;
            Ok(val.to_json())
        }
    }

    fn clean_up_expired(con: &mut Connection, key: String, member: String) -> Result<()> {
        CacheOperator::delete_expire(con, key, member)
    }

    fn clean_up_expired_by_key(con: &mut Connection, expired_key: String) -> Result<String> {
        if hexists(con, evict_to_rough_time(), expired_key.clone())? {
            let key = hget::<String, String>(con, evict_to_rough_time(), expired_key.clone())?;
            Self::clean_up_expired(con, key, expired_key.clone())?;
        }
        Ok(expired_key)
    }

    async fn try_lazy_evict(con: &mut Connection) -> Result<()> {
        let key = current_clean_up_key();
        if exists(con, key.clone())? {
            for member in smembers::<String>(con, key.clone())? {
                if get::<String, String>(con, member.clone()).is_err() {
                    info!("lazy evict key: {}", member);
                }
                Self::clean_up_expired(con, key.clone(), member.clone())?;
            }
        }
        Ok(())
    }

    async fn sub_evict_event(redis_con: &mut Connection) -> Result<()> {
        psubscribe(redis_con, EXPIRED_KEY_EVENT_AT_ALL_DB.to_string(), |msg| {
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
            match Self::clean_up_expired_by_key(&mut get_con(), expired_key) {
                Ok(expired_key) => info!("evict expired key: {}", expired_key),
                Err(e) => warn!("evict expired failed: {}", e),
            }
            ControlFlow::Continue
        })?;
        Ok(())
    }

    fn set_up(con: &mut Connection) -> Result<()> {
        for item in [
            CITA_CLOUD_BLOCK_NUMBER.to_string(),
            SYSTEM_CONFIG.to_string(),
        ] {
            let member = key_without_param(item);
            delete(con, member.clone())?;
            if Self::clean_up_expired_by_key(con, member.clone()).is_ok() {
                info!("set up -> reset key: {} success", member);
            }
        }
        let current = current_rough_time();
        for key in keys::<String>(con, clean_up_pattern())? {
            let rough_time_str: &str = &key[clean_up_prefix().len()..];
            if let Ok(rough_time) = rough_time_str.parse::<u64>() {
                if rough_time < current {
                    if let Ok(members) = smembers::<String>(con, key) {
                        for member in members {
                            if Self::clean_up_expired_by_key(con, member.clone()).is_ok() {
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

pub struct CacheOnly;

#[tonic::async_trait]
impl CacheBehavior for CacheOnly {}

#[tonic::async_trait]
pub trait MasterBehavior {
    async fn enqueue_raw_tx<S>(
        con: &mut Connection,
        signer: &S,
        raw_tx: CloudNormalTransaction,
    ) -> Result<Hash>
    where
        S: SignerBehaviour + Send + Sync;

    async fn enqueue_local_raw_tx<S>(
        con: &mut Connection,
        signer: &S,
        raw_tx: CloudNormalTransaction,
    ) -> Result<Hash>
    where
        S: SignerBehaviour + Send + Sync;
    async fn commit(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()>;

    async fn check(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()>;

    async fn sub_enqueue_stream(con: &mut Connection, timing_batch: usize) -> Result<()>;

    async fn sub_expire_stream(
        con: &mut Connection,
        time_internal: u64,
        timing_batch: usize,
    ) -> Result<()>;
    async fn save_block(con: &mut Connection, block: Vec<u8>) -> Result<Vec<u8>>;
    async fn get_block(hash: Vec<u8>) -> Result<Vec<u8>>;
}

#[derive(Clone, Copy)]
pub struct Master;

#[tonic::async_trait]
impl MasterBehavior for Master {
    async fn enqueue_raw_tx<S>(
        con: &mut Connection,
        signer: &S,
        raw_tx: CloudNormalTransaction,
    ) -> Result<Hash>
    where
        S: SignerBehaviour + Send + Sync,
    {
        let valid_until_block = raw_tx.valid_until_block;
        let mut buf = vec![];
        let raw = signer.sign_raw_tx(raw_tx, true).await;

        let empty = Vec::new();
        let hash = match raw.tx {
            Some(Tx::NormalTx(ref normal_tx)) => &normal_tx.transaction_hash,
            Some(Tx::UtxoTx(ref utxo_tx)) => &utxo_tx.transaction_hash,
            None => empty.as_slice(),
        };
        raw.encode(&mut buf)?;
        let hash_str = hex_without_0x(hash);
        CacheOperator::save_valid_until_block(con, hash_str.clone(), valid_until_block)?;
        CacheOperator::enqueue_tx(con, hash_str, buf)?;
        Ok(Hash::try_from_slice(hash)?)
    }

    // #[instrument(skip_all)]
    async fn enqueue_local_raw_tx<S>(
        con: &mut Connection,
        signer: &S,
        raw_tx: CloudNormalTransaction,
    ) -> Result<Hash>
    where
        S: SignerBehaviour + Send + Sync,
    {
        let raw = signer.sign_raw_tx(raw_tx, false).await;
        let mut buf = Vec::with_capacity(raw.encoded_len());
        raw.encode(&mut buf)?;
        let empty = Vec::new();
        let hash = match raw.tx {
            Some(Tx::NormalTx(ref normal_tx)) => &normal_tx.transaction_hash,
            Some(Tx::UtxoTx(ref utxo_tx)) => &utxo_tx.transaction_hash,
            None => empty.as_slice(),
        };
        let list = vec![("data".to_string(), buf.as_slice())];

        xadd::<&[u8]>(
            con,
            stream_key(ENQUEUE.to_string()),
            "*".to_string(),
            list.as_slice(),
        )?;
        Ok(Hash::try_from_slice(hash)?)
    }

    async fn commit(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()> {
        let members = CacheOperator::uncommitted_txs(con, timing_batch / 10)?;
        for (tx_hash, score) in members {
            let tx = CacheOperator::original_tx(con, tx_hash.clone())?;
            if tx.is_empty() {
                //package_tx without atomicity, fast continue
                continue;
            }
            match layer1().send_transaction(tx).await {
                Ok(data) => {
                    CacheOperator::commit_tx(con, tx_hash.clone(), score)?;
                    let hash_str = hex_without_0x(data.as_slice());
                    info!("commit tx success, hash: {}", hash_str);
                }
                Err(e) => {
                    warn!("commit tx failed, hash: {}, e: {}", tx_hash, e);
                    if Self::is_packaged_tx(con, tx_hash.clone())? {
                        let tx = CacheOperator::original_tx(con, tx_hash.clone())?;
                        let decoded: RawTransaction = Message::decode(tx.as_slice())?;
                        if let Some(Tx::NormalTx(normal_tx)) = decoded.tx {
                            let package_data =
                                normal_tx.transaction.expect("get transaction failed!").data;
                            let decoded_package = deserialize::<Package>(package_data.as_slice())?;
                            let maybe: MaybeLocked = BlockContext::current_account(con)?;
                            let account = maybe.unlocked()?;
                            let new_package = decoded_package
                                .to_packaged_tx(con, *account.address())
                                .await?;
                            let raw_tx = new_package.to(con).await?;
                            let new_hash = Self::enqueue_raw_tx(con, account, raw_tx).await?;
                            warn!(
                                "repackage batch: {}, new hash: {}",
                                decoded_package.batch_number,
                                hex_without_0x(new_hash.clone().as_slice())
                            );

                            CacheOperator::clean_up_tx(con, tx_hash)?;
                            Self::tag_tx(con, hex_without_0x(new_hash.as_slice()))?;
                        }
                    } else {
                        Self::save_error(
                            con,
                            tx_hash.clone(),
                            format!("{e}").as_bytes().to_vec(),
                            expire_time * 5,
                        )?;
                        CacheOperator::clean_up_tx(con, tx_hash.clone())?;
                        warn!("commit tx failed, hash: {}, e: {}", tx_hash, e);
                    }
                }
            }
        }
        Ok(())
    }

    async fn check(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()> {
        let members = CacheOperator::committed_txs(con, timing_batch)?;
        let layer1 = layer1();
        for (tx_hash, _) in members {
            let hash = parse_hash(tx_hash.clone().as_str())?;
            let (receipt, expire_time, is_ok) = match layer1.get_receipt(hash).await {
                Ok(receipt) => {
                    info!("get tx receipt success, hash: {}", tx_hash.clone());
                    (receipt, expire_time, true)
                }
                Err(e) => {
                    info!("retry -> get receipt, hash: {}, e: {}", tx_hash.clone(), e);
                    (format!("{e}").as_bytes().to_vec(), expire_time * 5, false)
                }
            };
            Self::save_receipt_content(con, tx_hash.clone(), receipt, expire_time)?;
            if is_ok {
                let (tx, expire_time) = match layer1.get_transaction(hash).await {
                    Ok(tx) => {
                        info!("get tx success, hash: {}", tx_hash.clone());
                        (tx, expire_time)
                    }
                    Err(e) => {
                        info!("retry -> get tx, hash: {}, e: {}", tx_hash.clone(), e);
                        (format!("{e}").as_bytes().to_vec(), expire_time * 5)
                    }
                };
                Self::save_tx_content(con, tx_hash.clone(), tx, expire_time)?;
                if Self::is_packaged_tx(con, tx_hash.clone())? {
                    for tx_hash in CacheOnly::get_block_txs(con, tx_hash.clone())? {
                        let ctx = CacheOnly::get_trace_ctx(con, tx_hash)?;
                        on_save_to_chain(ctx);
                    }
                }
                CacheOperator::try_clean_contract_data(con, tx_hash.clone())?;
                CacheOperator::clean_up_tx(con, tx_hash.clone())?;

                continue;
            }
            if check_if_timeout(con, tx_hash.clone()).await? {
                if Self::is_packaged_tx(con, tx_hash.clone())? {
                    let tx = CacheOperator::original_tx(con, tx_hash.clone())?;
                    let decoded: RawTransaction = Message::decode(tx.as_slice())?;
                    if let Some(Tx::NormalTx(normal_tx)) = decoded.tx {
                        let package_data =
                            normal_tx.transaction.expect("get transaction failed!").data;
                        let decoded_package = deserialize::<Package>(package_data.as_slice())?;
                        let maybe: MaybeLocked = BlockContext::current_account(con)?;
                        let account = maybe.unlocked()?;
                        let new_package = decoded_package
                            .to_packaged_tx(con, *account.address())
                            .await?;
                        let raw_tx = new_package.to(con).await?;
                        let new_hash = Self::enqueue_raw_tx(con, account, raw_tx).await?;
                        CacheOperator::clean_up_tx(con, tx_hash.clone())?;
                        Self::tag_tx(con, hex_without_0x(new_hash.as_slice()))?;
                        warn!("timeout repackage batch: {}.", decoded_package.batch_number);
                    }
                } else {
                    Self::save_error(
                        con,
                        tx_hash.clone(),
                        "timeout".to_string().as_bytes().to_vec(),
                        expire_time * 5,
                    )?;
                    CacheOperator::clean_up_tx(con, tx_hash.clone())?;
                    warn!("retry -> get receipt, timeout hash: {}", tx_hash);
                }
            }
        }
        Ok(())
    }

    async fn sub_enqueue_stream(con: &mut Connection, timing_batch: usize) -> Result<()> {
        let enqueue_id = get::<String, String>(con, stream_id_key(ENQUEUE.to_string()))
            .unwrap_or("0".to_string());
        let opts = StreamReadOptions::default().count(timing_batch);
        let results: StreamReadReply =
            con.xread_options(&[stream_key(ENQUEUE.to_string())], &[enqueue_id], opts)?;
        if results.keys.is_empty() {
            return Ok(());
        }
        for item in results.keys {
            if stream_key_suffix(item.key).as_str() == ENQUEUE {
                if item.ids.is_empty() {
                    continue;
                }
                let last = item.ids.last().unwrap().clone();
                let maybe: MaybeLocked = BlockContext::current_account(con)?;
                let account = maybe.unlocked()?;
                let proposer = account.address().to_vec();
                let mut tx_list = Vec::new();

                for id in item.ids {
                    let map = id.map;
                    match map.get("data") {
                        Some(RedisValue::Data(data)) => {
                            let decoded: RawTransaction = Message::decode(data.as_slice())?;
                            tx_list.push(decoded);
                        }
                        _ => continue,
                    };
                }
                let block =
                    BlockContext::fake_block(con, proposer.clone(), tx_list.clone()).await?;
                let size = tx_list.len();
                if let Ok(HashResponse {
                    status: Some(status),
                    hash: Some(state_root),
                }) = local_executor().exec(block.clone()).await
                {
                    if status.code == 0 {
                        let batch_number = BlockContext::get_batch_number(con).await?;

                        let packaged_tx_obj = Package::new(batch_number, block.clone())
                            .to_packaged_tx(con, *account.address())
                            .await?;
                        let raw_tx = packaged_tx_obj.to(con).await?;
                        warn!("raw_tx len: {}", raw_tx.encoded_len());
                        let hash = Self::enqueue_raw_tx(con, account, raw_tx).await?;
                        let hash_str = hex_without_0x(hash.as_slice());
                        for raw_tx in tx_list {
                            if let Some(Tx::NormalTx(normal_tx)) = raw_tx.clone().tx {
                                let tx_hash_str = hex_without_0x(&normal_tx.transaction_hash);
                                CacheOnly::save_block_tx(
                                    con,
                                    hash_str.clone(),
                                    tx_hash_str.clone(),
                                )?;
                                let ctx = CacheOnly::get_trace_ctx(con, tx_hash_str)?;
                                on_local_execute(ctx);
                            }

                            CacheOperator::try_clean_contract(con, raw_tx)?;
                        }
                        Self::tag_tx(con, hash_str.clone())?;

                        let header = block.header.expect("get block header failed");
                        let mut block_header_bytes = Vec::with_capacity(header.encoded_len());
                        header
                            .encode(&mut block_header_bytes)
                            .expect("encode block header failed");
                        let block_hash = account.hash(block_header_bytes.as_slice());
                        BlockContext::step_next(con, block_hash, state_root.hash)?;
                        set(con, stream_id_key(ENQUEUE.to_string()), last.id)?;
                        warn!(
                            "package batch: {}, txs_num: {}, hash: {}",
                            batch_number, size, hash_str
                        );
                    }
                }
            }
        }
        Ok(())
    }

    async fn sub_expire_stream(
        con: &mut Connection,
        time_internal: u64,
        timing_batch: usize,
    ) -> Result<()> {
        let expire_id = get::<String, String>(con, stream_id_key(EXPIRE.to_string()))
            .unwrap_or("0".to_string());
        let opts = if time_internal == 0 {
            StreamReadOptions::default().count(timing_batch)
        } else {
            StreamReadOptions::default()
                .block(time_internal as usize)
                .count(timing_batch)
        };
        let results: StreamReadReply =
            con.xread_options(&[stream_key(EXPIRE.to_string())], &[expire_id], opts)?;
        if results.keys.is_empty() {
            return Ok(());
        }
        for item in results.keys {
            if stream_key_suffix(item.key).as_str() == EXPIRE {
                for id in item.ids {
                    let map = id.map;
                    let (key, expire_time) = match map.get("data") {
                        Some(RedisValue::Data(data)) => {
                            let expire = deserialize::<Expire>(data.as_slice())?;
                            (expire.key, expire.expire_time)
                        }
                        _ => continue,
                    };
                    CacheOperator::update_expire(con, key.clone(), expire_time as usize)?;
                    set(con, stream_id_key(EXPIRE.to_string()), id.id)?;
                }
            }
        }
        Ok(())
    }

    async fn save_block(con: &mut Connection, block: Vec<u8>) -> Result<Vec<u8>> {
        let maybe: MaybeLocked = BlockContext::current_account(con)?;
        let account = maybe.unlocked()?;
        let hash = account.hash(block.as_slice());
        das()
            .put(
                save_block_key(hex_without_0x(hash.as_slice())).encode_to_vec(),
                block,
            )
            .await?;
        Ok(hash)
    }

    async fn get_block(hash: Vec<u8>) -> Result<Vec<u8>> {
        Ok(das()
            .get(save_block_key(hex_without_0x(hash.as_slice())).encode_to_vec())
            .await?)
    }
}

#[tonic::async_trait]
impl PackBehavior for Master {
    fn is_packaged_tx(con: &mut Connection, hash: String) -> Result<bool> {
        Ok(sismember(con, packaged_tx(), hash)?)
    }

    fn tag_tx(con: &mut Connection, hash: String) -> Result<()> {
        sadd(con, packaged_tx(), hash)?;
        Ok(())
    }
}

impl Master {
    pub fn save_tx_content(
        con: &mut Connection,
        tx_hash: String,
        content: Vec<u8>,
        expire_time: usize,
    ) -> Result<()> {
        CacheOperator::save_val(con, key(TX.to_string(), tx_hash), content, expire_time)?;
        Ok(())
    }

    pub fn save_receipt_content(
        con: &mut Connection,
        tx_hash: String,
        content: Vec<u8>,
        expire_time: usize,
    ) -> Result<()> {
        CacheOperator::save_val(con, key(RECEIPT.to_string(), tx_hash), content, expire_time)?;
        Ok(())
    }

    pub fn save_error(
        con: &mut Connection,
        hash: String,
        err: Vec<u8>,
        expire_time: usize,
    ) -> Result<()> {
        Self::save_tx_content(con, hash.clone(), err.clone(), expire_time)?;
        Self::save_receipt_content(con, hash, err, expire_time)
    }
}

#[tonic::async_trait]
pub trait ValidatorBehavior {
    fn enqueue_to_buffer(
        con: &mut Connection,
        hash_str: String,
        package_data: Vec<u8>,
        batch_number: u64,
    ) -> Result<()>;
    fn dequeue_smallest_from_buffer(con: &mut Connection) -> Result<Vec<(String, u64)>>;
    fn clean(con: &mut Connection, member: String) -> Result<()>;
    fn clean_up_packaged_txs(con: &mut Connection, hash_list: Vec<String>) -> Result<()>;
    async fn poll(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()>;
    async fn replay(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()>;
}

#[derive(Clone)]
pub struct Validator;

impl Validator {}

#[tonic::async_trait]
impl ValidatorBehavior for Validator {
    fn enqueue_to_buffer(
        con: &mut Connection,
        hash_str: String,
        package_data: Vec<u8>,
        batch_number: u64,
    ) -> Result<()> {
        info!(
            "enqueue tx to validator buffer, hash: {}, batch number: {}",
            hash_str, batch_number
        );
        zadd(con, validate_tx_buffer(), hash_str.clone(), batch_number)?;
        hset(con, hash_to_tx(), hash_str, package_data)?;
        Ok(())
    }

    fn dequeue_smallest_from_buffer(con: &mut Connection) -> Result<Vec<(String, u64)>> {
        Ok(zrange_withscores::<String>(
            con,
            validate_tx_buffer(),
            0,
            0,
        )?)
    }

    fn clean(con: &mut Connection, member: String) -> Result<()> {
        zrem(con, validate_tx_buffer(), member.clone())?;
        hdel(con, hash_to_tx(), member)?;
        Ok(())
    }

    fn clean_up_packaged_txs(con: &mut Connection, hash_list: Vec<String>) -> Result<()> {
        for hash in hash_list {
            zrem(con, pack_uncommitted_tx_key(), hash)?;
        }
        Ok(())
    }

    async fn poll(con: &mut Connection, _timing_batch: isize, _expire_time: usize) -> Result<()> {
        let layer1 = layer1();
        let account = BlockContext::current_account(con)?;
        let cita_height = BlockContext::current_cita_height(con)?;
        let key = validator_batch_number();
        let validator_current_height = if exists(con, key.clone())? {
            get(con, key.clone())?
        } else {
            incr_one(con, key.clone())?
        };
        if validator_current_height >= cita_height {
            return Ok(());
        }
        info!("validate cita cloud height: {}", validator_current_height);
        let compact_block = layer1.get_block_by_number(validator_current_height).await?;
        let compact_block: CompactBlock = Message::decode(compact_block.as_slice())?;
        let tx_hashs: Vec<Vec<u8>> = compact_block
            .body
            .expect("get compact body failed!")
            .tx_hashes;
        for hash in tx_hashs {
            info!(
                "validate cita cloud block [{}], has package txs",
                validator_current_height
            );
            if let Some((hash_str, package_data, batch_number)) = layer1
                .get_transaction_and_try_decode(
                    Hash::try_from_slice(hash.as_slice())?,
                    account.address().to_vec(),
                )
                .await?
            {
                Self::enqueue_to_buffer(con, hash_str, package_data, batch_number)?;
            }
        }
        incr_one(con, key)?;
        Ok(())
    }

    async fn replay(con: &mut Connection, _timing_batch: isize, _expire_time: usize) -> Result<()> {
        for (member, batch_number) in Self::dequeue_smallest_from_buffer(con)? {
            let current = BlockContext::get_batch_number(con).await?;
            match batch_number.cmp(&current) {
                //greater bacth_number enqueue to replay in order
                Ordering::Greater => {}
                //master commit tx which maybe fail, ignore repeat tx with lower batch_number
                Ordering::Less => Self::clean(con, member)?,
                Ordering::Equal => {
                    let maybe = BlockContext::current_account(con)?;
                    let account = maybe.unlocked()?;

                    let raw = CacheOperator::original_tx(con, member.clone())?;
                    let decoded_package = deserialize::<Package>(raw.as_slice())?;

                    let block: Block = Message::decode(decoded_package.block().as_slice())?;

                    let mut header = block.header.expect("get block header failed");
                    let body = block.body.clone().expect("get block body failed").clone();
                    let len = body.body.len();
                    header.prevhash = BlockContext::get_fake_block_hash(con).await?;
                    info!("replay batch: {} with {} txs!", batch_number, len);

                    let first = timestamp();
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
                        warn!("replay exec block cost {} ms!", timestamp() - first);

                        if let HashResponse {
                            status: Some(status),
                            hash: Some(state_root),
                        } = res
                        {
                            if status.code == 0 {
                                for raw_tx in block.body.expect("get block body failed").body {
                                    CacheOperator::try_clean_contract(con, raw_tx)?;
                                }
                                let mut block_header_bytes =
                                    Vec::with_capacity(header.encoded_len());
                                header
                                    .encode(&mut block_header_bytes)
                                    .expect("encode block header failed");
                                let block_hash = account.hash(block_header_bytes.as_slice());
                                BlockContext::step_next(con, block_hash, state_root.hash)?;
                                Self::clean(con, member)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
