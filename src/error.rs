use std::num::ParseIntError;

#[derive(Debug, thiserror::Error)]
pub enum ValidateError {
    #[error("parse address error")]
    ParseAddress(#[from] anyhow::Error),
    #[error("parse hash error")]
    ParseHash,
    #[error("parse int error")]
    ParseInt(#[from] ParseIntError),
    #[error("connect redis error")]
    Connect(#[from] r2d2::Error),
    #[error("operate redis command error")]
    Operate(#[from] r2d2_redis::redis::RedisError),
}
