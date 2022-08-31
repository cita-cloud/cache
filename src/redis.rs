extern crate redis;
use redis::{Connection, RedisResult};

pub fn connection() -> RedisResult<Connection> {
    let client = redis::Client::open("redis://default:rivtower@127.0.0.1:6379/")?;
    let connection = client.get_connection()?;
    Ok(connection)
}
