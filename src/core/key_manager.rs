use crate::common::constant::{
    rough_internal, COMMITTED_TX, CONTRACT_KEY, EVICT_TO_ROUGH_TIME, HASH_TO_BLOCK_NUMBER,
    HASH_TO_TX, HASH_TYPE, KEY_PREFIX, LAZY_EVICT_TO_TIME, ONE_THOUSAND, SET_TYPE,
    TIME_TO_CLEAN_UP, UNCOMMITTED_TX, VAL_TYPE, ZSET_TYPE,
};
use crate::common::util::{hex_without_0x, parse_data, timestamp};
use crate::redis::{sadd, smove, ttl};
use crate::{
    delete, exists, get, hdel, hget, hset, keys, smembers, srem, zadd, zrange_withscores, zrem,
    Display, RECEIPT, TX,
};
use anyhow::Result;
use cita_cloud_proto::blockchain::raw_transaction::Tx;
use cita_cloud_proto::blockchain::RawTransaction;
use prost::Message;
use r2d2_redis::redis::{FromRedisValue, ToRedisArgs};
use serde_json::Value;
use std::future::Future;

fn uncommitted_tx_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, UNCOMMITTED_TX)
}

fn committed_tx_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, COMMITTED_TX)
}

fn hash_to_tx() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, HASH_TO_TX)
}

fn hash_to_block_number() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, HASH_TO_BLOCK_NUMBER)
}

fn contract_pattern(to: String) -> String {
    format!("{}:{}:{}*", val_prefix(), CONTRACT_KEY, to)
}

fn clean_up_key(time: u64) -> String {
    format!("{}:{}:{}:{}", KEY_PREFIX, SET_TYPE, TIME_TO_CLEAN_UP, time)
}

fn lazy_evict_to_time() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, LAZY_EVICT_TO_TIME)
}

fn evict_to_rough_time() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, EVICT_TO_ROUGH_TIME)
}

pub fn val_prefix() -> String {
    format!("{}:{}", KEY_PREFIX, VAL_TYPE)
}

pub fn key(key_type: String, param: String) -> String {
    format!("{}:{}:{}", val_prefix(), key_type, param)
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
    fn enqueue(hash_str: String, tx_str: String) -> Result<()>;

    fn commit(tx_hash: String, score: u64) -> Result<()>;

    fn original_tx(tx_hash: String) -> Result<String>;

    fn valid_until_block(tx_hash: String) -> Result<u64>;

    fn save_valid_until_block(tx_hash: String, valid_until_block: u64) -> Result<u64>;

    fn uncommitted_txs(size: isize) -> Result<Vec<(String, u64)>, r2d2_redis::redis::RedisError>;

    fn committed_txs(size: isize) -> Result<Vec<(String, u64)>, r2d2_redis::redis::RedisError>;

    fn clean_up_tx(tx_hash: String) -> Result<()>;
}

#[tonic::async_trait]
pub trait ContractBehavior {
    fn try_clean_contract_data(tx_hash: String) -> Result<()>;
}

#[tonic::async_trait]
pub trait CacheBehavior: ExpiredBehavior + ValBehavior + ContractBehavior {
    async fn load_or_query<F, T>(key: String, expire_time: usize, f: F) -> Result<Value>
    where
        T: Display,
        F: Send + Future<Output = Result<T>>;

    fn save_tx(tx_hash: String, tx: String, expire_time: usize) -> Result<()>;

    fn save_receipt(tx_hash: String, tx: String, expire_time: usize) -> Result<()>;

    fn save_error(hash: String, err_str: String, expire_time: usize) -> Result<()>;

    fn clean_up_expired_by_key(expired_key: String) -> Result<String>;

    fn set_ex<T: Clone + Default + FromRedisValue + ToRedisArgs>(
        key: String,
        val: T,
        seconds: usize,
    ) -> Result<String>;

    fn expire(key: String, seconds: usize) -> Result<u64>;

    fn clean_up_expired(key: String, member: String) -> Result<()>;

    fn try_lazy_evict(time: u64) -> Result<()>;
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

    fn update_expire(key: String, seconds: usize) -> Result<()> {
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
    fn enqueue(hash_str: String, tx_str: String) -> Result<()> {
        zadd(uncommitted_tx_key(), hash_str.clone(), timestamp())?;
        hset(hash_to_tx(), hash_str, tx_str)?;
        Ok(())
    }

    fn commit(tx_hash: String, score: u64) -> Result<()> {
        zrem(uncommitted_tx_key(), tx_hash.clone())?;
        zadd(committed_tx_key(), tx_hash, score)?;
        Ok(())
    }

    fn original_tx(tx_hash: String) -> Result<String> {
        let tx = hget::<String>(hash_to_tx(), tx_hash)?;
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

    fn uncommitted_txs(size: isize) -> Result<Vec<(String, u64)>, r2d2_redis::redis::RedisError> {
        zrange_withscores::<String>(uncommitted_tx_key(), 0, size)
    }

    fn committed_txs(size: isize) -> Result<Vec<(String, u64)>, r2d2_redis::redis::RedisError> {
        zrange_withscores::<String>(committed_tx_key(), 0, size)
    }

    fn clean_up_tx(tx_hash: String) -> Result<()> {
        zrem(committed_tx_key(), tx_hash.clone())?;
        zrem(uncommitted_tx_key(), tx_hash.clone())?;
        hdel(hash_to_tx(), tx_hash.clone())?;
        hdel(hash_to_block_number(), tx_hash)?;
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
        let decoded: RawTransaction = Message::decode(parse_data(tx.as_str())?.as_slice())?;
        if let Some(Tx::NormalTx(normal_tx)) = decoded.tx {
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
impl CacheBehavior for CacheManager {
    async fn load_or_query<F, T>(key: String, expire_time: usize, f: F) -> Result<Value>
    where
        T: Display,
        F: Send + Future<Output = Result<T>>,
    {
        let result = if Self::exist_val(key.clone())? {
            Self::load_val(key, expire_time)
        } else {
            let val: T = f.await?;
            let display = val.display();
            Self::save_val(key, display.clone(), expire_time)?;
            Ok(display)
        }?;
        match serde_json::from_str(result.as_str()) {
            Ok(data) => Ok(data),
            Err(_) => Ok(Value::String(result)),
        }
    }

    fn save_tx(tx_hash: String, tx: String, expire_time: usize) -> Result<()> {
        Self::set_ex(key(TX.to_string(), tx_hash), tx, expire_time)?;
        Ok(())
    }

    fn save_receipt(tx_hash: String, tx: String, expire_time: usize) -> Result<()> {
        Self::set_ex(key(RECEIPT.to_string(), tx_hash), tx, expire_time)?;
        Ok(())
    }

    fn save_error(hash: String, err_str: String, expire_time: usize) -> Result<()> {
        Self::save_tx(hash.clone(), err_str.clone(), expire_time)?;
        Self::save_receipt(hash, err_str, expire_time)?;
        Ok(())
    }

    fn clean_up_expired_by_key(expired_key: String) -> Result<String> {
        let key = hget(evict_to_rough_time(), expired_key.clone())?;
        Self::clean_up_expired(key, expired_key.clone())?;
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

    fn try_lazy_evict(time: u64) -> Result<()> {
        let key = clean_up_key(time);
        if exists(key.clone())? {
            for member in smembers::<String>(key.clone())? {
                if get(member.clone()).is_err() {
                    info!("lazy evict key: {}", member);
                }
                Self::clean_up_expired(key.clone(), member.clone())?;
            }
        }
        Ok(())
    }
}
