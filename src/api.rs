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

use crate::constant::{FAILURE, SUCCESS, SUCCESS_MESSAGE};
use crate::display::Display;
use anyhow::{Error, Result};
use rocket::serde::Serialize;
use utoipa::Component;
use utoipa::OpenApi;

use crate::util::{key, key_without_param, parse_addr, parse_hash, parse_u64};
use cita_cloud_proto::{
    blockchain::{
        raw_transaction::Tx, Block, BlockHeader, RawTransaction, RawTransactions, Transaction,
        UnverifiedTransaction, Witness,
    },
    common::{NodeInfo, NodeNetInfo, TotalNodeInfo},
    controller::SystemConfig,
    evm::{Balance, ByteAbi, ByteCode, Log, Nonce, Receipt},
};
use redis::{Commands, FromRedisValue, RedisError, ToRedisArgs};
use rocket::Request;

use rocket::request::{FromRequest, Outcome};
use rocket::serde::json::Json;
use serde_json::Value;

fn try_load<T: FromRedisValue + ToRedisArgs + Clone>(
    key: String,
    default_val: T,
) -> Result<T, RedisError> {
    let mut con = crate::redis::connection()?;
    if con.exists(key.clone())? {
        let block_number: T = con.get(key)?;
        Ok(block_number)
    } else {
        con.set(key, default_val.clone())?;
        Ok(default_val)
    }
}

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
impl<'r> FromRequest<'r> for ValidateResult<u64, RedisError> {
    type Error = RedisError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pattern = get_param(req, 0);
        let (key, default_val) = match pattern {
            "get-block-number" => (key_without_param("block-number"), 1),
            "get-peers-count" => (key_without_param("peers-count"), 4),
            "get-version" => (key_without_param("version"), 1),
            _ => return Outcome::Forward(()),
        };
        match try_load(key, default_val) {
            Ok(number) => Outcome::Success(ValidateResult::Ok(number)),
            Err(e) => Outcome::Success(ValidateResult::Err(e)),
        }
    }
}
#[derive(Debug)]
pub enum ParseObjError {
    RedisErr(RedisError),
    DeSerializeErr(serde_json::Error),
}

impl std::fmt::Display for ParseObjError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::RedisErr(e) => write!(f, "{}", e),
            Self::DeSerializeErr(e) => write!(f, "{}", e),
        }
    }
}
#[rocket::async_trait]
impl<'r> FromRequest<'r> for ValidateResult<Value, ParseObjError> {
    type Error = ParseObjError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pattern = get_param(req, 0);
        let (key, default_val) = match pattern {
            "get-peers-info" => {
                let nodes = vec![NodeInfo {
                    address: vec![0u8; 32],
                    net_info: Some(NodeNetInfo {
                        multi_address: "".to_string(),
                        origin: 1,
                    }),
                }];
                (
                    key_without_param("peers-info"),
                    TotalNodeInfo { nodes }.display(),
                )
            }
            "get-system-config" => {
                let validators = vec![vec![0u8; 32]];
                (
                    key_without_param("system-config"),
                    SystemConfig {
                        version: 1,
                        chain_id: vec![0u8; 48],
                        validators,
                        admin: vec![0u8; 32],
                        block_interval: 3,
                        emergency_brake: true,
                        version_pre_hash: vec![0u8; 48],
                        chain_id_pre_hash: vec![0u8; 48],
                        admin_pre_hash: vec![0u8; 48],
                        block_interval_pre_hash: vec![0u8; 48],
                        validators_pre_hash: vec![0u8; 48],
                        emergency_brake_pre_hash: vec![0u8; 48],
                        quota_limit: 1073741824,
                        quota_limit_pre_hash: vec![0u8; 48],
                        block_limit: 100,
                        block_limit_pre_hash: vec![0u8; 48],
                    }
                    .display(),
                )
            }
            _ => return Outcome::Forward(()),
        };
        match try_load(key, default_val) {
            Ok(val) => match serde_json::from_str(val.as_str()) {
                Ok(json) => Outcome::Success(ValidateResult::Ok(json)),
                Err(e) => Outcome::Success(ValidateResult::Err(ParseObjError::DeSerializeErr(e))),
            },
            Err(e) => Outcome::Success(ValidateResult::Err(ParseObjError::RedisErr(e))),
        }
    }
}

#[derive(Debug)]
pub enum AddressError {
    RedisErr(RedisError),
    ParseAddrErr(Error),
}

impl std::fmt::Display for AddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::RedisErr(e) => write!(f, "{}", e),
            Self::ParseAddrErr(_) => write!(f, "ParseAddrErr"),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ValidateResult<String, AddressError> {
    type Error = AddressError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pattern = get_param(req, 0);
        let param_str = get_param(req, 1);
        if pattern != "get-block-hash" {
            if let Err(e) = parse_addr(param_str) {
                return Outcome::Success(ValidateResult::Err(AddressError::ParseAddrErr(e)));
            }
        }
        let uri_str = req.uri().to_string();
        let pattern = get_param(req, 0);
        let (key_type, default_val) = match pattern {
            "get-abi" => (
                "abi",
                ByteAbi {
                    bytes_abi: vec![0u8; 32],
                }
                .display(),
            ),
            "get-balance" => (
                "balance",
                Balance {
                    value: vec![0u8; 32],
                }
                .display(),
            ),
            "get-account-nonce" => (
                "account-nonce",
                Nonce {
                    nonce: vec![0u8; 32],
                }
                .display(),
            ),
            "get-code" => (
                "code",
                ByteCode {
                    byte_code: vec![0u8; 32],
                }
                .display(),
            ),
            "get-block-hash" => {
                let hash: [u8; 32] = [0; 32];
                ("block-hash", hash.display())
            }
            _ => return Outcome::Forward(()),
        };
        println!(
            "uri: {}, key_type: {}, default_val: {}",
            uri_str, key_type, default_val
        );
        let key = key(key_type, param_str);
        match try_load(key, default_val) {
            Ok(val) => Outcome::Success(ValidateResult::Ok(val)),
            Err(e) => Outcome::Success(ValidateResult::Err(AddressError::RedisErr(e))),
        }
    }
}

#[derive(Debug)]
pub enum ObjError {
    Redis(RedisError),
    ParseHash(Error),
    DeSerialize(serde_json::Error),
}

impl std::fmt::Display for ObjError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Redis(e) => write!(f, "{}", e),
            Self::ParseHash(_) => write!(f, "ParseHashErr"),
            Self::DeSerialize(_) => write!(f, "DeSerializeErr"),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ValidateResult<Value, ObjError> {
    type Error = ObjError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let uri_str = req.uri().to_string();
        let pattern = get_param(req, 0);
        let param_str = get_param(req, 1);
        if pattern == "get-block" {
            if let Ok(height) = parse_u64(param_str) {
                println!("{}", height);
            } else if let Err(e) = parse_hash(param_str) {
                return Outcome::Success(ValidateResult::Err(ObjError::ParseHash(e)));
            }
        } else if let Err(e) = parse_hash(param_str) {
            return Outcome::Success(ValidateResult::Err(ObjError::ParseHash(e)));
        }

        let (key_type, default_val) = match pattern {
            "get-receipt" => {
                let topics = vec![vec![0u8; 32]];
                let logs = vec![Log {
                    address: vec![0u8; 32],
                    topics,
                    data: vec![0u8; 32],
                    block_hash: vec![0u8; 32],
                    block_number: 1,
                    transaction_hash: vec![0u8; 32],
                    transaction_index: 1,
                    log_index: 1,
                    transaction_log_index: 1,
                }];
                (
                    "receipt",
                    Receipt {
                        transaction_hash: vec![0u8; 32],
                        transaction_index: 1,
                        block_hash: vec![0u8; 32],
                        block_number: 1,
                        cumulative_quota_used: vec![0u8; 32],
                        quota_used: vec![0u8; 32],
                        contract_address: vec![0u8; 32],
                        logs,
                        state_root: vec![0u8; 32],
                        logs_bloom: vec![0u8; 32],
                        error_message: "".to_string(),
                    }
                    .display(),
                )
            }
            "get-tx" => {
                let raw_tx = {
                    let witness = Witness {
                        sender: vec![0u8; 32],
                        signature: vec![0u8; 32],
                    };

                    let unverified_tx = UnverifiedTransaction {
                        transaction: Some(Transaction {
                            to: vec![0u8; 32],
                            data: vec![0u8; 32],
                            value: vec![0u8; 32],
                            nonce: String::from(""),
                            quota: 1073741824,
                            valid_until_block: 5,
                            chain_id: vec![0u8; 64],
                            version: 0,
                        }),
                        transaction_hash: vec![0u8; 32],
                        witness: Some(witness),
                    };

                    RawTransaction {
                        tx: Some(Tx::NormalTx(unverified_tx)),
                    }
                };
                ("tx", (raw_tx, 1, 1).display())
            }
            "get-block" => {
                let header = BlockHeader {
                    prevhash: vec![0u8; 32],
                    timestamp: 1660724911266,
                    height: 0,
                    transactions_root: vec![0u8; 32],
                    proposer: vec![0u8; 32],
                };
                let raw_tx = {
                    let witness = Witness {
                        sender: vec![0u8; 32],
                        signature: vec![0u8; 32],
                    };

                    let unverified_tx = UnverifiedTransaction {
                        transaction: Some(Transaction {
                            to: vec![0u8; 32],
                            data: vec![0u8; 32],
                            value: vec![0u8; 32],
                            nonce: String::from(""),
                            quota: 1073741824,
                            valid_until_block: 5,
                            chain_id: vec![0u8; 64],
                            version: 0,
                        }),
                        transaction_hash: vec![0u8; 32],
                        witness: Some(witness),
                    };

                    RawTransaction {
                        tx: Some(Tx::NormalTx(unverified_tx)),
                    }
                };
                let txs = vec![raw_tx];
                let block = Block {
                    version: 0,
                    header: Some(header),
                    body: Some(RawTransactions { body: txs }),
                    proof: vec![0u8; 64],
                };
                ("block", block.display())
            }
            _ => return Outcome::Forward(()),
        };
        println!(
            "uri: {}, key_type: {}, default_val: {}",
            uri_str, key_type, default_val
        );
        let key = key(key_type, param_str);
        match try_load(key, default_val) {
            Ok(val) => match serde_json::from_str(val.as_str()) {
                Ok(json) => Outcome::Success(ValidateResult::Ok(json)),
                Err(e) => Outcome::Success(ValidateResult::Err(ObjError::DeSerialize(e))),
            },
            Err(e) => Outcome::Success(ValidateResult::Err(ObjError::Redis(e))),
        }
    }
}

#[catch(404)]
pub fn uri_not_found() -> Json<String> {
    Json(String::from("URI not found"))
}

#[catch(404)]
pub fn api_not_found() -> Json<String> {
    Json(String::from("api not found"))
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
pub struct QueryResult<T> {
    pub status: u64,
    pub data: Option<T>,
    pub message: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
#[component(example = json!({"status": 1, "message": "success"}))]
pub struct SuccessResult {
    pub status: u64,
    pub message: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
#[component(example = json!({"status": 0, "message": "error message"}))]
pub struct FailureResult {
    pub status: u64,
    pub message: String,
}

fn match_result<T, E: std::fmt::Display>(result: ValidateResult<T, E>) -> Json<QueryResult<T>> {
    match result {
        ValidateResult::Ok(num) => Json(QueryResult {
            status: SUCCESS,
            data: Some(num),
            message: SUCCESS_MESSAGE.to_string(),
        }),
        ValidateResult::Err(e) => Json(QueryResult {
            status: FAILURE,
            data: None,
            message: format!("{}", e),
        }),
    }
}

///Get current block number
#[get("/get-block-number")]
#[utoipa::path(get, path = "/api/get-block-number")]
pub fn block_number(result: ValidateResult<u64, RedisError>) -> Json<QueryResult<u64>> {
    match_result(result)
}

///Get contract abi by contract address
#[get("/get-abi/<address>")]
#[utoipa::path(
    get,
    path = "/api/get-abi/{address}",
    params(
        ("address" = String, path, description = "The contract address"),
    )
)]
pub fn abi(
    address: &str,
    result: ValidateResult<String, AddressError>,
) -> Json<QueryResult<String>> {
    println!("get-abi address {}", address);
    match_result(result)
}

///Get balance by account address
#[get("/get-balance/<address>")]
#[utoipa::path(
    get,
    path = "/api/get-balance/{address}",
    params(
        ("address" = String, path, description = "The account address"),
    )
)]
pub fn balance(
    address: &str,
    result: ValidateResult<String, AddressError>,
) -> Json<QueryResult<String>> {
    println!("get-balance address {}", address);
    match_result(result)
}

///Get block by height or hash
#[get("/get-block/<hash_or_height>")]
#[utoipa::path(
    get,
    path = "/api/get-block/{hash_or_height}",
    params(
        ("hash_or_height" = String, path, description = "The block hash or height"),
    )
)]
pub fn block(
    hash_or_height: &str,
    result: ValidateResult<Value, ObjError>,
) -> Json<QueryResult<Value>> {
    println!("get-block hash_or_height {}", hash_or_height);
    match_result(result)
}

///Get code by contract address
#[get("/get-code/<address>")]
#[utoipa::path(
    get,
    path = "/api/get-code/{address}",
    params(
        ("address" = String, path, description = "The contract address"),
    )
)]
pub fn code(
    address: &str,
    result: ValidateResult<String, AddressError>,
) -> Json<QueryResult<String>> {
    println!("get-code address {}", address);
    match_result(result)
}

///Get tx by hash
#[get("/get-tx/<hash>")]
#[utoipa::path(
    get,
    path = "/api/get-tx/{hash}",
    params(
        ("hash" = String, path, description = "The tx hash"),
    )
)]
pub fn tx(hash: &str, result: ValidateResult<Value, ObjError>) -> Json<QueryResult<Value>> {
    println!("get-tx hash {}", hash);
    match_result(result)
}

///Get peers count
#[get("/get-peers-count")]
#[utoipa::path(get, path = "/api/get-peers-count")]
pub fn peers_count(result: ValidateResult<u64, RedisError>) -> Json<QueryResult<u64>> {
    match_result(result)
}

///Get peers info
#[get("/get-peers-info")]
#[utoipa::path(get, path = "/api/get-peers-info")]
pub fn peers_info(result: ValidateResult<Value, ParseObjError>) -> Json<QueryResult<Value>> {
    match_result(result)
}

///Get nonce by account address
#[get("/get-account-nonce/<address>")]
#[utoipa::path(
    get,
    path = "/api/get-account-nonce/{address}",
    params(
        ("address" = String, path, description = "The account address"),
    )
)]
pub fn account_nonce(
    address: &str,
    result: ValidateResult<String, AddressError>,
) -> Json<QueryResult<String>> {
    println!("get-account-nonce address {}", address);
    match_result(result)
}

///Get tx receipt by hash
#[get("/get-receipt/<hash>")]
#[utoipa::path(
    get,
    path = "/api/get-receipt/{hash}",
    params(
        ("hash" = String, path, description = "The tx hash"),
    )
)]
pub fn receipt(hash: &str, result: ValidateResult<Value, ObjError>) -> Json<QueryResult<Value>> {
    println!("get-receipt hash {}", hash);
    match_result(result)
}

///Get chain version
#[get("/get-version")]
#[utoipa::path(get, path = "/api/get-version")]
pub fn version(result: ValidateResult<u64, RedisError>) -> Json<QueryResult<u64>> {
    match_result(result)
}

///Get system config
#[get("/get-system-config")]
#[utoipa::path(get, path = "/api/get-system-config")]
pub fn system_config(result: ValidateResult<Value, ParseObjError>) -> Json<QueryResult<Value>> {
    match_result(result)
}

///Get block hash by block number
#[get("/get-block-hash/<block_number>")]
#[utoipa::path(
    get,
    path = "/api/get-block-hash/{block_number}",
    params(
        ("block_number" = usize, path, description = "The block number"),
    )
)]
pub fn block_hash(
    block_number: usize,
    result: ValidateResult<String, AddressError>,
) -> Json<QueryResult<String>> {
    println!("get-block-hash block_number {}", block_number);
    match_result(result)
}
#[derive(OpenApi)]
#[openapi(
    handlers(
        block_number,
        abi,
        balance,
        block,
        code,
        tx,
        peers_count,
        peers_info,
        account_nonce,
        receipt,
        system_config,
        block_hash,
        version
    ),
    components(SuccessResult, FailureResult)
)]
pub struct ApiDoc;
