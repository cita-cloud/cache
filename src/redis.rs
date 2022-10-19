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

const REDIS_ADDRESS: &str = "redis://default:rivtower@127.0.0.1:6379";

// Pool initiation.
// Call it starting an app and store a pool as a rocket managed state.
pub fn pool() -> Pool {
    let manager = RedisConnectionManager::new(REDIS_ADDRESS).expect("connection manager");
    Pool::new(manager).expect("db pool")
}

pub type Pool = r2d2::Pool<RedisConnectionManager>;

pub fn load(
    mut con: PooledConnection<RedisConnectionManager>,
    key: String,
) -> Result<String, r2d2_redis::redis::RedisError> {
    if con.exists(key.clone())? {
        let data: String = con.get(key)?;
        Ok(data)
    } else {
        Ok(String::default())
    }
}

// pub fn exists(mut con: PooledConnection<RedisConnectionManager>,
//               key: String,) -> Result<bool, r2d2_redis::redis::RedisError> {
//     con.exists(key)
// }

pub fn set<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    mut con: PooledConnection<RedisConnectionManager>,
    key: String,
    val: T,
) -> Result<String, r2d2_redis::redis::RedisError> {
    con.set::<String, T, String>(key, val)
}

// pub fn delete(
//     mut con: PooledConnection<RedisConnectionManager>,
//     key: String,
// ) -> Result<String, r2d2_redis::redis::RedisError> {
//     con.del(key)
// }

pub fn hset(
    mut con: PooledConnection<RedisConnectionManager>,
    hkey: String,
    key: String,
    val: String,
) -> Result<u64, r2d2_redis::redis::RedisError> {
    con.hset::<String, String, String, u64>(hkey, key, val)
}

pub fn hget<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    mut con: PooledConnection<RedisConnectionManager>,
    hkey: String,
    key: T,
) -> Result<String, r2d2_redis::redis::RedisError> {
    con.hget(hkey, key)
}

pub fn zadd<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    mut con: PooledConnection<RedisConnectionManager>,
    zkey: String,
    member: T,
    score: u64
) -> Result<String, r2d2_redis::redis::RedisError> {
    con.zadd(zkey, member, score)
}


pub fn zrange<T: Clone + Default + FromRedisValue + ToRedisArgs>(
    mut con: PooledConnection<RedisConnectionManager>,
    zkey: String,
    start: isize,
    stop: isize,
) -> Result<Vec<T>, r2d2_redis::redis::RedisError> {
    con.zrange(zkey, start, stop)
}
