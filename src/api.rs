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

use crate::constant::{SUCCESS, SUCCESS_MESSAGE};
use crate::display::Display;
use rocket::serde::{json::Json, Serialize};
use serde_json::Value;
use utoipa::Component;
use utoipa::OpenApi;

use cita_cloud_proto::{
    blockchain::{
        raw_transaction::Tx, Block, BlockHeader, RawTransaction, RawTransactions, Transaction,
        UnverifiedTransaction, Witness,
    },
    common::{NodeInfo, NodeNetInfo, TotalNodeInfo},
    controller::SystemConfig,
    evm::{Balance, ByteAbi, ByteCode, Log, Nonce, Receipt},
};

extern crate rocket;

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
    pub data: T,
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

///Get current block number
#[get("/get-block-number")]
#[utoipa::path(get, path = "/api/get-block-number")]
pub fn block_number() -> Json<QueryResult<u64>> {
    Json(QueryResult {
        status: SUCCESS,
        data: 1,
        message: SUCCESS_MESSAGE.to_string(),
    })
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
pub fn abi(address: &str) -> Json<QueryResult<String>> {
    println!("get-abi address {}", address);
    Json(QueryResult {
        status: SUCCESS,
        data: ByteAbi {
            bytes_abi: vec![0u8; 32],
        }
        .display(),
        message: SUCCESS_MESSAGE.to_string(),
    })
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
pub fn balance(address: &str) -> Json<QueryResult<Value>> {
    println!("get-balance address {}", address);
    Json(QueryResult {
        status: SUCCESS,
        data: Balance {
            value: vec![0u8; 32],
        }
        .to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
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
pub fn block(hash_or_height: &str) -> Json<QueryResult<Value>> {
    println!("get-block hash_or_height {}", hash_or_height);
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
    Json(QueryResult {
        status: SUCCESS,
        data: block.to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
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
pub fn code(address: &str) -> Json<QueryResult<Value>> {
    println!("get-code address {}", address);

    Json(QueryResult {
        status: SUCCESS,
        data: ByteCode {
            byte_code: vec![0u8; 32],
        }
        .to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
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
pub fn tx(hash: &str) -> Json<QueryResult<Value>> {
    println!("get-tx hash {}", hash);

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

    Json(QueryResult {
        status: SUCCESS,
        data: (raw_tx, 1, 1).to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
}

///Get peers count
#[get("/get-peers-count")]
#[utoipa::path(get, path = "/api/get-peers-count")]
pub fn peers_count() -> Json<QueryResult<u64>> {
    Json(QueryResult {
        status: SUCCESS,
        data: 4,
        message: SUCCESS_MESSAGE.to_string(),
    })
}

///Get peers info
#[get("/get-peers-info")]
#[utoipa::path(get, path = "/api/get-peers-info")]
pub fn peers_info() -> Json<QueryResult<Value>> {
    let nodes = vec![NodeInfo {
        address: vec![0u8; 32],
        net_info: Some(NodeNetInfo {
            multi_address: "".to_string(),
            origin: 1,
        }),
    }];
    Json(QueryResult {
        status: SUCCESS,
        data: TotalNodeInfo { nodes }.to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
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
pub fn account_nonce(address: &str) -> Json<QueryResult<Value>> {
    println!("get-account-nonce address {}", address);

    Json(QueryResult {
        status: SUCCESS,
        data: Nonce {
            nonce: vec![0u8; 32],
        }
        .to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
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
pub fn receipt(hash: &str) -> Json<QueryResult<Value>> {
    println!("get-receipt hash {}", hash);

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
    Json(QueryResult {
        status: SUCCESS,
        data: Receipt {
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
        .to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
}

///Get chain version
#[get("/get-version")]
#[utoipa::path(get, path = "/api/get-version")]
pub fn version() -> Json<QueryResult<String>> {
    Json(QueryResult {
        status: SUCCESS,
        data: "1".to_string(),
        message: SUCCESS_MESSAGE.to_string(),
    })
}

///Get system config
#[get("/get-system-config")]
#[utoipa::path(get, path = "/api/get-system-config")]
pub fn system_config() -> Json<QueryResult<Value>> {
    let validators = vec![vec![0u8; 32]];
    Json(QueryResult {
        status: SUCCESS,
        data: SystemConfig {
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
        .to_json(),
        message: SUCCESS_MESSAGE.to_string(),
    })
}

///Get block hash by block number
#[get("/get-block-hash/<block_number>")]
#[utoipa::path(
    get,
    path = "/api/get-block-hash/{block_number}",
    params(
        ("address" = String, path, description = "The block number"),
    )
)]
pub fn block_hash(block_number: &str) -> Json<QueryResult<String>> {
    println!("get-block-hash block_number {}", block_number);

    let hash: [u8; 32] = [0; 32];
    Json(QueryResult {
        status: SUCCESS,
        data: hash.display(),
        message: SUCCESS_MESSAGE.to_string(),
    })
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
