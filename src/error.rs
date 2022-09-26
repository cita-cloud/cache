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
    #[error("operate redis command error: {0}")]
    Operate(#[from] r2d2_redis::redis::RedisError),
    #[error("deserialize json error")]
    Deserialize(#[from] serde_json::error::Error),
    #[error("get {get_type:?} error: {detail:?}")]
    Getdata {
        get_type: String,
        detail: anyhow::Error,
    },
}
