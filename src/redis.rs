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
use crate::common::constant::REDIS_POOL;
use r2d2::PooledConnection;
use r2d2_redis::redis::{Commands, ControlFlow, FromRedisValue, Msg, PubSubCommands, ToRedisArgs};
use r2d2_redis::RedisConnectionManager;
use std::collections::HashSet;
use std::fmt::Display;
use std::hash::Hash;

// Pool initiation.
// Call it starting an app and store a pool as a rocket managed state.
pub fn pool(redis_addr: String, workers: u32) -> Pool {
    let manager = RedisConnectionManager::new(redis_addr).expect("connection manager");
    Pool::builder()
        .max_size(workers)
        .min_idle(Some(10))
        .build(manager)
        .expect("db pool")
}

pub type Connection = PooledConnection<RedisConnectionManager>;

pub type Pool = r2d2::Pool<RedisConnectionManager>;

pub fn con() -> Connection {
    REDIS_POOL.get().unwrap().get().unwrap()
}

pub fn get<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    con: &mut Connection,
    key: String,
) -> Result<T, r2d2_redis::redis::RedisError> {
    con.get(key)
}

pub fn incr_one(con: &mut Connection, key: String) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.incr::<String, u64, u64>(key, 1)
}

pub fn ttl(con: &mut Connection, key: String) -> Result<isize, r2d2_redis::redis::RedisError> {
    con.ttl(key)
}

pub fn expire(
    con: &mut Connection,
    key: String,
    expire_time: usize,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.expire(key, expire_time)
}

#[allow(dead_code)]
pub fn exists(con: &mut Connection, key: String) -> Result<bool, r2d2_redis::redis::RedisError> {
    con.exists(key)
}

#[allow(dead_code)]
pub fn set_nx<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    con: &mut Connection,
    key: String,
    val: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.set_nx(key, val)
}

#[allow(dead_code)]
pub fn set<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    con: &mut Connection,
    key: String,
    val: T,
) -> Result<String, r2d2_redis::redis::RedisError> {
    con.set::<String, T, String>(key, val)
}

pub fn set_ex<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    con: &mut Connection,
    key: String,
    val: T,
    seconds: usize,
) -> Result<String, r2d2_redis::redis::RedisError> {
    con.set_ex::<String, T, String>(key, val, seconds)
}

#[allow(dead_code)]
pub fn delete<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    con: &mut Connection,
    key: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.del(key)
}

pub fn hset<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    con: &mut Connection,
    hkey: String,
    key: String,
    val: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.hset::<String, String, T, u64>(hkey, key, val)
}

pub fn hget<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    hkey: String,
    key: String,
) -> Result<T, r2d2_redis::redis::RedisError> {
    con.hget(hkey, key)
}

#[allow(dead_code)]
pub fn hvals<T: Clone + Default + ToRedisArgs + Display + FromRedisValue + Eq + Hash>(
    con: &mut Connection,
    hkey: String,
) -> Result<HashSet<T>, r2d2_redis::redis::RedisError> {
    con.hvals(hkey)
}

pub fn hexists(
    con: &mut Connection,
    hkey: String,
    key: String,
) -> Result<bool, r2d2_redis::redis::RedisError> {
    con.hexists(hkey, key)
}

#[allow(dead_code)]
pub fn hdel<T: Clone + Default + ToRedisArgs>(
    con: &mut Connection,
    hkey: String,
    key: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.hdel(hkey, key)
}

#[allow(dead_code)]
pub fn hkeys<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    hkey: String,
) -> Result<Vec<T>, r2d2_redis::redis::RedisError> {
    con.hkeys(hkey)
}

pub fn zadd<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    con: &mut Connection,
    zkey: String,
    member: T,
    score: u64,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.zadd(zkey, member, score)
}

pub fn zrem<T: Clone + Default + ToRedisArgs>(
    con: &mut Connection,
    zkey: String,
    member: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.zrem(zkey, member)
}

#[allow(dead_code)]
pub fn zrange<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    zkey: String,
    start: isize,
    stop: isize,
) -> Result<Vec<T>, r2d2_redis::redis::RedisError> {
    con.zrange(zkey, start, stop)
}

pub fn zrange_withscores<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    zkey: String,
    start: isize,
    stop: isize,
) -> Result<Vec<(T, u64)>, r2d2_redis::redis::RedisError> {
    con.zrange_withscores(zkey, start, stop)
}

#[allow(dead_code)]
pub fn sadd<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    key: String,
    member: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.sadd(key, member)
}

#[allow(dead_code)]
pub fn sismember<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    key: String,
    member: T,
) -> Result<bool, r2d2_redis::redis::RedisError> {
    con.sismember(key, member)
}

pub fn smembers<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    key: String,
) -> Result<Vec<T>, r2d2_redis::redis::RedisError> {
    con.smembers(key)
}

pub fn smove<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    src: String,
    target: String,
    member: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.smove(src, target, member)
}

pub fn srem<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    key: String,
    member: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.srem(key, member)
}

pub fn keys<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    con: &mut Connection,
    pattern: String,
) -> Result<Vec<T>, r2d2_redis::redis::RedisError> {
    con.keys(pattern)
}

pub fn psubscribe<F: FnMut(Msg) -> ControlFlow<()>>(
    con: &mut Connection,
    pattern: String,
    func: F,
) -> Result<(), r2d2_redis::redis::RedisError> {
    con.psubscribe(pattern, func)
}

#[allow(dead_code)]
pub fn publish(
    con: &mut Connection,
    channel: String,
    msg: String,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.publish(channel, msg)
}
