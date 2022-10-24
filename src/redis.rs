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
extern crate rocket;

use r2d2::PooledConnection;
use r2d2_redis::redis::{Commands, FromRedisValue, ToRedisArgs};
use r2d2_redis::RedisConnectionManager;
use tokio::sync::OnceCell;

pub static REDIS_POOL: OnceCell<Pool> = OnceCell::const_new();
// Pool initiation.
// Call it starting an app and store a pool as a rocket managed state.
pub fn pool(redis_addr: String) -> Pool {
    let manager = RedisConnectionManager::new(redis_addr).expect("connection manager");
    let pool = Pool::new(manager).expect("db pool");
    match REDIS_POOL.set(pool.clone()) {
        Ok(_) => {}
        Err(e) => println!("set redis pool fail: {:?}", e),
    };
    pool
}

pub type Pool = r2d2::Pool<RedisConnectionManager>;

pub fn con() -> PooledConnection<RedisConnectionManager> {
    REDIS_POOL.get().unwrap().get().unwrap()
}

pub fn load(key: String) -> Result<String, r2d2_redis::redis::RedisError> {
    if con().exists(key.clone())? {
        let data: String = con().get(key)?;
        Ok(data)
    } else {
        Ok(String::default())
    }
}
#[allow(dead_code)]
pub fn exists(key: String) -> Result<bool, r2d2_redis::redis::RedisError> {
    con().exists(key)
}

pub fn set<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    key: String,
    val: T,
) -> Result<String, r2d2_redis::redis::RedisError> {
    con().set::<String, T, String>(key, val)
}

#[allow(dead_code)]
pub fn delete(key: String) -> Result<String, r2d2_redis::redis::RedisError> {
    con().del(key)
}

pub fn hset(hkey: String, key: String, val: String) -> Result<u64, r2d2_redis::redis::RedisError> {
    con().hset::<String, String, String, u64>(hkey, key, val)
}

#[allow(dead_code)]
pub fn hget<T: Clone + Default + ToRedisArgs>(
    hkey: String,
    key: T,
) -> Result<String, r2d2_redis::redis::RedisError> {
    con().hget(hkey, key)
}

pub fn hdel<T: Clone + Default + ToRedisArgs>(
    hkey: String,
    key: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con().hdel(hkey, key)
}

pub fn hkeys<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    hkey: String,
) -> Result<Vec<T>, r2d2_redis::redis::RedisError> {
    con().hkeys(hkey)
}

pub fn zadd<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    zkey: String,
    member: T,
    score: u64,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con().zadd(zkey, member, score)
}

pub fn zrem<T: Clone + Default + ToRedisArgs>(
    zkey: String,
    member: T,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con().zrem(zkey, member)
}

pub fn zrange<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    zkey: String,
    start: isize,
    stop: isize,
) -> Result<Vec<T>, r2d2_redis::redis::RedisError> {
    con().zrange(zkey, start, stop)
}

pub fn zrange_withscores<T: Clone + Default + ToRedisArgs + FromRedisValue>(
    zkey: String,
    start: isize,
    stop: isize,
) -> Result<Vec<(T, u64)>, r2d2_redis::redis::RedisError> {
    con().zrange_withscores(zkey, start, stop)
}
