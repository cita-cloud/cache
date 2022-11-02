use crate::common::constant::{
    COMMITTED_TX, CONTRACT_KEY, EVICT_TO_ROUGH_TIME, HASH_TO_BLOCK_NUMBER, HASH_TO_TX, HASH_TYPE,
    KEY_PREFIX, LAZY_EVICT_TO_TIME, SET_TYPE, TIME_TO_CLEAN_UP, UNCOMMITTED_TX, VAL_TYPE,
    ZSET_TYPE,
};
use crate::common::util::{display_time, timestamp};
use crate::redis::{sadd, smove};
use crate::{exists, hdel, hget, hset, srem, zrem, ROUGH_INTERNAL};
use anyhow::Result;
use r2d2_redis::redis::{FromRedisValue, ToRedisArgs};

pub fn rough_internal() -> u64 {
    *ROUGH_INTERNAL.get().unwrap()
}

pub fn val_prefix() -> String {
    format!("{}:{}", KEY_PREFIX, VAL_TYPE)
}

pub fn key(key_type: String, param: String) -> String {
    format!("{}:{}:{}", val_prefix(), key_type, param)
}

pub fn key_without_param(key_type: &str) -> String {
    format!("{}:{}", val_prefix(), key_type)
}

pub fn uncommitted_tx_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, UNCOMMITTED_TX)
}

pub fn committed_tx_key() -> String {
    format!("{}:{}:{}", KEY_PREFIX, ZSET_TYPE, COMMITTED_TX)
}

pub fn hash_to_tx() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, HASH_TO_TX)
}

pub fn hash_to_block_number() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, HASH_TO_BLOCK_NUMBER)
}

pub fn contract_pattern(to: String) -> String {
    format!("{}:{}:{}*", val_prefix(), CONTRACT_KEY, to)
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

pub fn clean_up_key(time: u64) -> String {
    format!("{}:{}:{}:{}", KEY_PREFIX, SET_TYPE, TIME_TO_CLEAN_UP, time)
}

pub fn lazy_evict_to_time() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, LAZY_EVICT_TO_TIME)
}

pub fn evict_to_rough_time() -> String {
    format!("{}:{}:{}", KEY_PREFIX, HASH_TYPE, EVICT_TO_ROUGH_TIME)
}

pub fn time_pair(timestamp: u64, internal: usize, rough_internal: u64) -> (u64, u64) {
    let expire_time = timestamp + (internal * 1000) as u64;
    info!("expire time: {}", display_time(expire_time));
    //key 之前 internal内过期的key
    let rough_time = rough_time(expire_time, rough_internal);
    info!("rough time: {}", display_time(rough_time));
    (expire_time, rough_time)
}

pub fn rough_time(expire_time: u64, rough_internal: u64) -> u64 {
    expire_time - expire_time % (rough_internal * 1000) + rough_internal * 1000
}

fn update_expire(key: String, seconds: usize) -> Result<()> {
    let old_expire_time = hget(lazy_evict_to_time(), key.clone())?;
    let rough_internal = rough_internal();
    let old_rough_time = rough_time(old_expire_time, rough_internal);

    let (expire_time, rough_time) = time_pair(timestamp(), seconds, rough_internal);

    smove(
        clean_up_key(old_rough_time),
        clean_up_key(rough_time),
        key.clone(),
    )?;
    hset(lazy_evict_to_time(), key.clone(), expire_time)?;
    hset(evict_to_rough_time(), key, clean_up_key(rough_time))?;
    Ok(())
}

fn create_expire(key: String, seconds: usize) -> Result<()> {
    let (expire_time, rough_time) = time_pair(timestamp(), seconds, rough_internal());
    sadd(clean_up_key(rough_time), key.clone())?;
    hset(lazy_evict_to_time(), key.clone(), expire_time)?;
    hset(evict_to_rough_time(), key, clean_up_key(rough_time))?;
    Ok(())
}

pub fn set_ex<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    key: String,
    val: T,
    seconds: usize,
) -> Result<String> {
    if exists(key.clone())? {
        update_expire(key.clone(), seconds)?;
    } else {
        create_expire(key.clone(), seconds)?;
    }
    let result = crate::redis::set_ex(key, val, seconds)?;
    Ok(result)
}

pub fn expire(key: String, seconds: usize) -> Result<u64> {
    update_expire(key.clone(), seconds)?;
    let result = crate::redis::expire(key, seconds)?;
    Ok(result)
}

pub fn clean_up_tx(tx_hash: String) -> Result<()> {
    zrem(committed_tx_key(), tx_hash.clone())?;
    zrem(uncommitted_tx_key(), tx_hash.clone())?;
    hdel(hash_to_tx(), tx_hash.clone())?;
    hdel(hash_to_block_number(), tx_hash)?;
    Ok(())
}

pub fn clean_up_expired(key: String, member: String) -> Result<()> {
    hdel(lazy_evict_to_time(), member.clone())?;
    hdel(evict_to_rough_time(), member.clone())?;
    srem(key, member)?;
    Ok(())
}

pub fn clean_up_expired_by_key(expired_key: String) -> Result<String> {
    let key = hget(evict_to_rough_time(), expired_key.clone())?;
    clean_up_expired(key, expired_key.clone())?;
    Ok(expired_key)
}
