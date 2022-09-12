extern crate rocket;

use std::ops::Deref;
use rocket::{http, Request};
use r2d2;
use r2d2_redis::redis::ConnectionLike;
use r2d2_redis::RedisConnectionManager;
use rocket::request::{FromRequest, Outcome};
use rocket::State;
use redis::{Connection, RedisError, RedisResult};

const REDIS_ADDRESS: &'static str = "redis://default:rivtower@127.0.0.1:6379";

// Pool initiation.
// Call it starting an app and store a pool as a rocket managed state.
pub fn pool() -> Pool {
    let manager = RedisConnectionManager::new(REDIS_ADDRESS).expect("connection manager");
    Pool::new(manager).expect("db pool")
}

pub type Pool = r2d2::Pool<RedisConnectionManager>;


