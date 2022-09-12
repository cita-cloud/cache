extern crate rocket;

use crate::constant::{FAILURE, SUCCESS, SUCCESS_MESSAGE};
use crate::display::Display;
use anyhow::{Error, Result};
use rocket::serde::Serialize;
use utoipa::Component;
use utoipa::OpenApi;

use crate::util::{key, key_without_param, parse_addr, parse_hash, parse_u64, remove_0x};
use cita_cloud_proto::{
    blockchain::{
        raw_transaction::Tx, Block, BlockHeader, RawTransaction, RawTransactions, Transaction,
        UnverifiedTransaction, Witness,
    },
    common::{NodeInfo, NodeNetInfo, TotalNodeInfo},
    controller::{SystemConfig, Flag},
    evm::{Balance, ByteAbi, ByteCode, Log, Nonce, Receipt},
};
use r2d2_redis::redis::ConnectionLike;
use r2d2_redis::redis::{ToRedisArgs, FromRedisValue};
use rocket::{Request, State};

use rocket::request::{FromRequest, Outcome};
use rocket::serde::json::Json;
use serde_json::Value;
use crate::redis::{Pool};
use r2d2_redis::redis::Commands;
use rocket::http::Status;
use r2d2::{Error as R2d2Err, PooledConnection};
use r2d2_redis::redis::RedisError as OperateErr;
use r2d2_redis::RedisConnectionManager;
use rocket::futures::TryFutureExt;
use crate::{ControllerClient, EvmClient, ExecutorClient};
use crate::context::Context;
use crate::core::controller::ControllerBehaviour;
use crate::error::ValidateError;
use std::string::String;
use crate::crypto::{Address, Hash};

fn get_param<'r>(req: &'r Request<'_>, index: usize) -> &'r str {
    if let Some(val) = req.param(index) {
        val.ok().unwrap()
    } else {
        ""
    }
}




#[derive(Debug)]
pub enum ValidateResult<T, E> {
    Ok(T),
    Err(E),
}



#[rocket::async_trait]
impl<'r> FromRequest<'r> for Context<ControllerClient, ExecutorClient, EvmClient> {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let ctx = req.guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient>>>().await.unwrap();
        Outcome::Success(Context {
            controller: ctx.controller.clone(),
            executor: ctx.executor.clone(),
            evm: ctx.evm.clone(),
            redis_pool: ctx.redis_pool.clone()
        })
    }
}



fn load<T: Clone + Default + FromRedisValue>(
    mut con: PooledConnection<RedisConnectionManager>,
    key: String,
) -> Result<T, r2d2_redis::redis::RedisError> {
    if con.exists(key.clone())? {
        let data: T = con.get(key)?;
        Ok(data)
    } else {
        Ok(T::default())
    }
}

#[derive(Debug)]
pub enum ParamType {
    Empty,
    Address(Address),
    Hash(Hash),
    Number(u64),
    NumOrHash(u64, Hash),
}


fn param_type(path: &str, param: &str) -> Result<ParamType, ValidateError> {
    match path {
        "get-block-number" |"get-peers-count"|"get-version"| "get-peers-info" | "get-system-config" => Ok(ParamType::Empty),
        "get-abi" |"get-account-nonce"|"get-balance" | "get-code" => match parse_addr(param) {
            Ok(data) => Ok(ParamType::Address(data)),
            Err(e) => Err(ValidateError::ParseAddress(e)),
        },
        "get-block-hash" => match parse_u64(param) {
            Ok(data) => Ok(ParamType::Number(data)),
            Err(e) => Err(ValidateError::ParseInt(e)),
        },
        "get-receipt" | "get-tx" => match parse_hash(param) {
            Ok(data) => Ok(ParamType::Hash(data)),
            Err(_) => Err(ValidateError::ParseHash),
        },
        _ => if let Ok(data) = parse_u64(param) {
            Ok(ParamType::NumOrHash(data, Hash::default()))
        } else  {
            match parse_hash(param) {
                Ok(data) => Ok(ParamType::NumOrHash(0, data)),
                Err(_) => Err(ValidateError::ParseHash),
            }
        },
    }
}
#[rocket::async_trait]
impl<'r> FromRequest<'r> for ValidateResult<Entity<String>, ValidateError> {
    type Error = ValidateError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pattern = get_param(req, 0);
        let param = remove_0x(get_param(req, 1));
        let param_type = param_type(pattern, param).unwrap();

        let ctx = req.guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient>>>().await.unwrap();
        let mut con = ctx.get_redis_connection();
        let pattern = &pattern[4..];

        let key = if param_type == ParamType::Empty {
            key(pattern, param)
        } else {
            key_without_param(pattern)
        };
        match load(con, key.clone()) {
            Ok(val) => Outcome::Success(ValidateResult::Ok(Entity {
                key,
                val,
                param: param_type
            })),
            Err(e) => Outcome::Success(ValidateResult::Err(ValidateError::Operate(e)))
        }
    }
}



#[derive(Debug)]
pub struct Entity<T: Default> {
    pub key: String,
    pub val: T,
    pub param: ParamType
}







