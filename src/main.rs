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

mod cache_log;
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

use crate::cache_log::LOGGER;
use crate::constant::{RECEIPT, TX};
use crate::context::Context;
use crate::core::controller::{ControllerBehaviour, ControllerClient};
use crate::core::crypto::CryptoClient;
use crate::core::evm::{EvmBehaviour, EvmClient};
use crate::core::executor::ExecutorClient;
use crate::crypto::{ArrayLike, Hash};
use crate::display::Display;
use crate::redis::{
    delete, hdel, hget, hset, keys, pool, set, zadd, zrange, zrange_withscores, zrem,
};
use crate::util::{
    committed_tx_key, contract_pattern, current_time, hash_to_block_number, hash_to_tx,
    hex_without_0x, init_local_utc_offset, key, parse_data, uncommitted_tx_key,
};
use ::log::LevelFilter;
use anyhow::{Error, Result};
use cita_cloud_proto::blockchain::{raw_transaction::Tx, RawTransaction};
use log::{set_logger, set_max_level};
use prost::Message;
use rest_api::common::{api_not_found, uri_not_found, ApiDoc};
use rest_api::get::{
    abi, account_nonce, balance, block, block_hash, block_number, code, peers_count, peers_info,
    receipt, system_config, tx, version,
};
use rest_api::post::{call, create, send_tx};
use rocket::config::LogLevel;
use rocket::fairing::AdHoc;
use rocket::{routes, Build, Rocket};
use serde::Deserialize;
use tokio::time;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[macro_use]
extern crate rocket;

fn rocket(figment: Figment) -> Rocket<Build> {
    rocket::custom(figment)
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
                call,
                create,
                send_tx,
            ],
        )
        .register("/", catchers![uri_not_found])
        .register("/api", catchers![api_not_found])
}

async fn commit_tx_schedule(
    timing_internal_sec: u64,
    timing_batch: isize,
    controller: ControllerClient,
) {
    let mut internal = time::interval(time::Duration::from_secs(timing_internal_sec));
    loop {
        internal.tick().await;
        if let Err(e) = commit_tx(timing_batch, controller.clone()).await {
            warn!("commit tx error: {}", e);
        }
    }
}

async fn commit_tx(timing_batch: isize, controller: ControllerClient) -> Result<()> {
    let members = zrange_withscores::<String>(uncommitted_tx_key(), 0, timing_batch)?;
    for (tx_hash, score) in members {
        let tx = match hget::<String>(hash_to_tx(), tx_hash.clone()) {
            Ok(data) => data,
            Err(e) => {
                warn!(
                    "hget hkey: {}, key: {}, error: {}",
                    hash_to_tx(),
                    tx_hash.clone(),
                    e
                );
                return Err(Error::from(e));
            }
        };
        let decoded: RawTransaction = Message::decode(&parse_data(tx.as_str())?[..])?;
        match controller.send_raw(decoded.clone()).await {
            Ok(data) => {
                zrem(uncommitted_tx_key(), tx_hash.clone())?;
                zadd(committed_tx_key(), tx_hash.clone(), score)?;
                let hash_str = hex_without_0x(&data);
                info!("commit tx success, hash: {}", hash_str);
            }
            Err(e) => {
                let err_str = format!("{}", e);
                let empty = Vec::new();
                let hash = if let Some(Tx::NormalTx(ref normal_tx)) = decoded.tx {
                    &normal_tx.transaction_hash
                } else {
                    empty.as_slice()
                };
                let hash = hex_without_0x(hash).to_string();
                zrem(uncommitted_tx_key(), tx_hash.clone())?;
                set(key(RECEIPT.to_string(), hash.clone()), err_str.clone())?;
                set(key(TX.to_string(), hash.clone()), err_str)?;
                warn!("commit tx fail, hash: {}", hash);
            }
        }
    }
    Ok(())
}

async fn clean_key(tx_hash: String) -> Result<()> {
    zrem(committed_tx_key(), tx_hash.clone())?;
    zrem(uncommitted_tx_key(), tx_hash.clone())?;
    hdel(hash_to_tx(), tx_hash.clone())?;
    hdel(hash_to_block_number(), tx_hash)?;
    Ok(())
}

async fn check_tx(timing_batch: isize, controller: ControllerClient, evm: EvmClient) -> Result<()> {
    let members = zrange::<String>(committed_tx_key(), 0, timing_batch)?;
    for tx_hash in members {
        let hash = Hash::try_from_slice(&parse_data(tx_hash.clone().as_str()).unwrap()[..])?;
        match evm.get_receipt(hash).await {
            Ok(receipt) => {
                set(key(RECEIPT.to_string(), tx_hash.clone()), receipt.display())?;
                info!("get tx receipt and save success, hash: {}", tx_hash.clone());
                match controller.get_tx(hash).await {
                    Ok(tx) => {
                        set(key(TX.to_string(), tx_hash.clone()), tx.display())?;
                        info!("get tx and save success, hash: {}", tx_hash.clone());
                    }
                    Err(e) => {
                        set(key(TX.to_string(), tx_hash.clone()), format!("{}", e))?;
                        info!("retry get tx, hash: {}", tx_hash.clone());
                    }
                }

                let tx = hget::<String>(hash_to_tx(), tx_hash.clone())?;
                let decoded: RawTransaction = Message::decode(parse_data(tx.as_str())?.as_slice())?;
                if let Some(Tx::NormalTx(normal_tx)) = decoded.tx {
                    if let Some(transaction) = normal_tx.transaction {
                        let to_addr = transaction.to;
                        let addr = hex_without_0x(&to_addr);
                        let pattern = contract_pattern(addr);
                        if let Ok(list) = keys::<String>(pattern) {
                            if !list.is_empty() {
                                delete(list)?;
                            }
                        }
                    }
                }
                clean_key(tx_hash.clone()).await?;
                continue;
            }
            Err(e) => {
                set(key(RECEIPT.to_string(), tx_hash.clone()), format!("{}", e))?;
                info!("retry -> get receipt, hash: {}", tx_hash.clone());
            }
        }
        let current = controller.get_block_number(false).await?;
        let config = controller.get_system_config().await?;
        let valid_until_block = hget::<u64>(hash_to_block_number(), tx_hash.clone())?;
        if valid_until_block <= current || valid_until_block > (current + config.block_limit as u64)
        {
            warn!("retry -> get receipt, timeout hash: {}", tx_hash.clone());
            set(
                key(RECEIPT.to_string(), tx_hash.clone()),
                "timeout".to_string(),
            )?;
            clean_key(tx_hash).await?
        }
    }
    Ok(())
}
async fn check_tx_schedule(
    timing_internal_sec: u64,
    timing_batch: isize,
    controller: ControllerClient,
    evm: EvmClient,
) {
    let mut internal = time::interval(time::Duration::from_secs(2 * timing_internal_sec));
    loop {
        internal.tick().await;
        if let Err(e) = check_tx(timing_batch, controller.clone(), evm.clone()).await {
            warn!("check tx error: {}", e);
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
    log_level: LogLevel,
}

use rocket::config::Config as RocketConfig;
use rocket::figment::providers::{Env, Format, Toml};
use rocket::figment::{Figment, Profile};

#[rocket::main]
async fn main() {
    init_local_utc_offset();
    let figment = Figment::from(RocketConfig::debug_default())
        .merge(Toml::file(Env::var_or("ROCKET_CONFIG", "Rocket.toml")).nested())
        .merge(Env::prefixed("ROCKET_").ignore(&["PROFILE"]).global())
        .select(Profile::from_env_or(
            "ROCKET_PROFILE",
            RocketConfig::DEFAULT_PROFILE,
        ));
    let config = figment.extract::<Config>().unwrap();
    match set_logger(&LOGGER) {
        Ok(_) => {}
        Err(e) => println!("{}", e),
    }
    set_max_level(LevelFilter::from(config.log_level));
    let rocket: Rocket<Build> = rocket(figment).attach(AdHoc::config::<Config>());

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
    let timing_internal_sec = config.timing_internal_sec.unwrap_or(1);
    let timing_batch = config.timing_batch.unwrap_or(100) as isize;
    tokio::spawn(commit_tx_schedule(
        timing_internal_sec,
        timing_batch,
        ctx.controller.clone(),
    ));
    tokio::spawn(check_tx_schedule(
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
