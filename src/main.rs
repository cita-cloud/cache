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

mod constant;
mod context;
mod core;
mod crypto;
mod display;
mod error;
mod from_request;
mod health_check;
mod redis;
mod rest_api;
mod util;

use crate::constant::{RECEIPT, TX};
use crate::context::Context;
use crate::core::account::Account;
use crate::core::controller::{ControllerBehaviour, ControllerClient, TransactionSenderBehaviour};
use crate::core::crypto::CryptoClient;
use crate::core::evm::{EvmBehaviour, EvmClient};
use crate::core::executor::ExecutorClient;
use crate::crypto::{Address, ArrayLike, Hash};
use crate::display::Display;
use crate::redis::{hdel, hget, hset, pool, set, zadd, zrange, zrange_withscores, zrem};
use crate::rest_api::post::new_raw_tx;
use crate::util::{
    committed_tx_key, current_time, hash_to_retry, hash_to_tx, hex_without_0x,
    init_local_utc_offset, key, parse_addr, parse_data, uncommitted_tx_key,
};
use anyhow::{Error, Result};
use cita_cloud_proto::blockchain::{raw_transaction::Tx, RawTransaction};
use prost::Message;
use rest_api::common::{api_not_found, uri_not_found, ApiDoc};
use rest_api::get::{
    abi, account_nonce, balance, block, block_hash, block_number, code, peers_count, peers_info,
    receipt, system_config, tx, version,
};
use rest_api::post::{create, send_tx};
use rocket::fairing::AdHoc;
use rocket::form::validate::Contains;
use rocket::{routes, Build, Rocket};
use serde::Deserialize;
use tokio::time;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[macro_use]
extern crate rocket;

fn rocket() -> Rocket<Build> {
    rocket::build()
        .mount(
            "/",
            SwaggerUi::new("/swagger-ui/<_..>").url("/api-doc/openapi.json", ApiDoc::openapi()),
        )
        .mount(
            "/api",
            routes![
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
                version,
                create,
                send_tx,
            ],
        )
        .register("/", catchers![uri_not_found])
        .register("/api", catchers![api_not_found])
}

async fn commit_tx(
    timing_internal_sec: u64,
    address: Address,
    timing_batch: isize,
    controller: ControllerClient,
    crypto: CryptoClient,
) -> Result<()> {
    let mut internal = time::interval(time::Duration::from_secs(timing_internal_sec));
    loop {
        internal.tick().await;
        let members = zrange_withscores::<String>(uncommitted_tx_key(), 0, timing_batch)?;
        for (tx_hash, score) in members {
            let real_hash = if let Ok(new_hash) = hget(hash_to_retry(), tx_hash.clone()) {
                new_hash
            } else {
                tx_hash.clone()
            };
            let tx = match hget(hash_to_tx(), real_hash.clone()) {
                Ok(data) => data,
                Err(e) => {
                    warn!(
                        "[{}] hget hkey: {}, key: {}, error: {}",
                        current_time(),
                        hash_to_tx(),
                        real_hash.clone(),
                        e
                    );
                    return Err(Error::from(e));
                }
            };
            let decoded: RawTransaction = Message::decode(&parse_data(tx.as_str()).unwrap()[..])?;
            match controller.send_raw(decoded.clone()).await {
                Ok(data) => {
                    zrem(uncommitted_tx_key(), tx_hash.clone())?;
                    zadd(committed_tx_key(), tx_hash.clone(), score)?;
                    let hash_str = hex_without_0x(&data);
                    info!("[{}] commit tx success, hash: {}", current_time(), hash_str);
                }
                Err(e) => {
                    let err_str = format!("{}", e);
                    if err_str.contains("InvalidValidUntilBlock") {
                        zadd(uncommitted_tx_key(), tx_hash.clone(), score)?;
                        if let Some(Tx::NormalTx(normal_tx)) = decoded.tx {
                            let raw_tx =
                                new_raw_tx(controller.clone(), normal_tx.transaction.unwrap())
                                    .await?;
                            let data = controller
                                .send_raw_tx(&Account::new(crypto.clone(), address), raw_tx.clone())
                                .await?;
                            let hash = hex_without_0x(data.as_slice());
                            hset(hash_to_retry(), tx_hash.clone(), hash.clone())?;
                            //avoid dup operation of a tx
                            zrem(uncommitted_tx_key(), hash.clone())?;
                            info!("[{}] recommit tx, new hash: {}", current_time(), hash);
                            continue;
                        }
                    }
                    let empty = Vec::new();
                    let hash = match decoded.tx {
                        Some(Tx::NormalTx(ref normal_tx)) => &normal_tx.transaction_hash,
                        Some(Tx::UtxoTx(ref utxo_tx)) => &utxo_tx.transaction_hash,
                        None => empty.as_slice(),
                    };
                    let hash = hex_without_0x(hash).to_string();
                    set(key(RECEIPT.to_string(), hash.clone()), err_str.clone())?;
                    set(key(TX.to_string(), hash.clone()), err_str)?;
                    zrem(uncommitted_tx_key(), tx_hash.clone())?;
                    warn!("[{}] commit tx fail, hash: {}", current_time(), hash);
                }
            }
        }
    }
}

async fn check_tx(
    timing_internal_sec: u64,
    timing_batch: isize,
    controller: ControllerClient,
    evm: EvmClient,
) -> Result<()> {
    let mut internal = time::interval(time::Duration::from_secs(2 * timing_internal_sec));
    loop {
        internal.tick().await;
        let members = zrange::<String>(committed_tx_key(), 0, timing_batch)?;
        for tx_hash in members {
            let real_hash = if let Ok(new_hash) = hget(hash_to_retry(), tx_hash.clone()) {
                new_hash
            } else {
                tx_hash.clone()
            };
            let hash = Hash::try_from_slice(&parse_data(real_hash.as_str()).unwrap()[..])?;
            match evm.get_receipt(hash).await {
                Ok(receipt) => {
                    set(key(RECEIPT.to_string(), tx_hash.clone()), receipt.display())?;
                    info!(
                        "[{}] get tx receipt and save success, hash: {}",
                        current_time(),
                        tx_hash.clone()
                    );
                    match controller.get_tx(hash).await {
                        Ok(tx) => {
                            set(key(TX.to_string(), tx_hash.clone()), tx.display())?;
                            info!(
                                "[{}] get tx and save success, hash: {}",
                                current_time(),
                                tx_hash.clone()
                            );
                        }
                        Err(e) => {
                            set(key(TX.to_string(), tx_hash.clone()), format!("{}", e))?;
                            info!(
                                "[{}] retry get tx, hash: {}",
                                current_time(),
                                tx_hash.clone()
                            );
                        }
                    }
                    zrem(committed_tx_key(), tx_hash.clone())?;
                    zrem(uncommitted_tx_key(), tx_hash.clone())?;
                    hdel(hash_to_tx(), tx_hash)?;
                }
                Err(e) => {
                    set(key(RECEIPT.to_string(), tx_hash.clone()), format!("{}", e))?;
                    info!(
                        "[{}] retry -> get receipt, hash: {}",
                        current_time(),
                        tx_hash.clone()
                    );
                }
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Config {
    controller_addr: Option<String>,
    executor_addr: Option<String>,
    crypto_addr: Option<String>,
    redis_addr: Option<String>,
    timing_internal_sec: Option<u64>,
    timing_batch: Option<u64>,
    account: String,
}

#[rocket::main]
async fn main() {
    init_local_utc_offset();
    let rocket: Rocket<Build> = rocket().attach(AdHoc::config::<Config>());
    let figment = rocket.figment();
    let config: Config = figment.extract().expect("config");

    let ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient> = Context::new(
        config
            .controller_addr
            .unwrap_or_else(|| "http://127.0.0.1:50004".to_string()),
        config
            .executor_addr
            .unwrap_or_else(|| "http://127.0.0.1:50002".to_string()),
        config
            .crypto_addr
            .unwrap_or_else(|| "http://127.0.0.1:50005".to_string()),
        config
            .redis_addr
            .unwrap_or_else(|| "redis://default:rivtower@127.0.0.1:6379".to_string()),
    );
    let address = parse_addr(config.account.as_str()).unwrap();
    let timing_internal_sec = config.timing_internal_sec.unwrap_or(1);
    let timing_batch = config.timing_batch.unwrap_or(100) as isize;
    tokio::spawn(commit_tx(
        timing_internal_sec,
        address,
        timing_batch,
        ctx.controller.clone(),
        ctx.crypto.clone(),
    ));
    tokio::spawn(check_tx(
        timing_internal_sec,
        timing_batch,
        ctx.controller.clone(),
        ctx.evm.clone(),
    ));
    if let Err(e) = rocket.manage(ctx).launch().await {
        error!("Whoops! Rocket didn't launch!");
        // We drop the error to get a Rocket-formatted panic.
        drop(e);
    };
}
