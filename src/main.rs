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

mod cita_cloud;
mod common;
mod core;
mod health_check;
mod redis;
mod rest_api;

use crate::cita_cloud::controller::{ControllerBehaviour, ControllerClient};
use crate::cita_cloud::crypto::CryptoClient;
use crate::cita_cloud::evm::{EvmBehaviour, EvmClient};
use crate::cita_cloud::executor::ExecutorClient;
use crate::common::cache_log::LOGGER;
use crate::common::constant::{RECEIPT, TX};
use crate::common::crypto::{ArrayLike, Hash};
use crate::common::display::Display;
use crate::common::util::{
    clean_up_key, committed_tx_key, contract_pattern, current_time, evict_key_to_time_key,
    hash_to_block_number, hash_to_tx, hex_without_0x, init_local_utc_offset, key, parse_data,
    timestamp, uncommitted_tx_key,
};
use crate::core::context::Context;
use crate::redis::{
    delete, exists, get, hdel, hget, hset, keys, pool, set_ex, smembers, zadd, zrange,
    zrange_withscores, zrem,
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
    expire_time: usize,
    timing_batch: isize,
    controller: ControllerClient,
) {
    let mut internal = time::interval(time::Duration::from_secs(timing_internal_sec));
    loop {
        internal.tick().await;
        if let Err(e) = commit_tx(timing_batch, expire_time, controller.clone()).await {
            warn!("commit tx error: {}", e);
        }
    }
}

async fn commit_tx(
    timing_batch: isize,
    expire_time: usize,
    controller: ControllerClient,
) -> Result<()> {
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
                set_ex(
                    key(RECEIPT.to_string(), hash.clone()),
                    err_str.clone(),
                    expire_time * 5,
                )?;
                set_ex(key(TX.to_string(), hash.clone()), err_str, expire_time * 5)?;
                clean_key(hash.clone()).await?;
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

async fn check_tx(
    timing_batch: isize,
    expire_time: usize,
    controller: ControllerClient,
    evm: EvmClient,
) -> Result<()> {
    let members = zrange::<String>(committed_tx_key(), 0, timing_batch)?;
    for tx_hash in members {
        let hash = Hash::try_from_slice(&parse_data(tx_hash.clone().as_str()).unwrap()[..])?;
        match evm.get_receipt(hash).await {
            Ok(receipt) => {
                set_ex(
                    key(RECEIPT.to_string(), tx_hash.clone()),
                    receipt.display(),
                    expire_time,
                )?;
                info!("get tx receipt and save success, hash: {}", tx_hash.clone());
                match controller.get_tx(hash).await {
                    Ok(tx) => {
                        set_ex(
                            key(TX.to_string(), tx_hash.clone()),
                            tx.display(),
                            expire_time * 5,
                        )?;
                        info!("get tx and save success, hash: {}", tx_hash.clone());
                    }
                    Err(e) => {
                        set_ex(
                            key(TX.to_string(), tx_hash.clone()),
                            format!("{}", e),
                            expire_time * 5,
                        )?;
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
                set_ex(
                    key(RECEIPT.to_string(), tx_hash.clone()),
                    format!("{}", e),
                    expire_time,
                )?;
                info!("retry -> get receipt, hash: {}", tx_hash.clone());
            }
        }
        let current = controller.get_block_number(false).await?;
        let config = controller.get_system_config().await?;
        let valid_until_block = hget::<u64>(hash_to_block_number(), tx_hash.clone())?;
        if valid_until_block <= current || valid_until_block > (current + config.block_limit as u64)
        {
            warn!("retry -> get receipt, timeout hash: {}", tx_hash.clone());
            set_ex(
                key(RECEIPT.to_string(), tx_hash.clone()),
                "timeout".to_string(),
                expire_time,
            )?;
            clean_key(tx_hash).await?
        }
    }
    Ok(())
}
async fn check_tx_schedule(
    timing_internal_sec: u64,
    timing_batch: isize,
    expire_time: usize,
    controller: ControllerClient,
    evm: EvmClient,
) {
    let mut internal = time::interval(time::Duration::from_secs(2 * timing_internal_sec));
    loop {
        internal.tick().await;
        if let Err(e) = check_tx(timing_batch, expire_time, controller.clone(), evm.clone()).await {
            warn!("check tx error: {}", e);
        }
    }
}

async fn check_expired_key(timing_internal_sec: u64, expire_time: usize) {
    let mut internal = time::interval(time::Duration::from_secs(2 * timing_internal_sec));
    loop {
        internal.tick().await;
        if let Err(e) = evict_expired_key(timing_internal_sec, expire_time).await {
            warn!("check expired key error: {}", e);
        }
    }
}

async fn evict_expired_key(timing_internal_sec: u64, expire_time: usize) -> Result<()> {
    let mut internal = time::interval(time::Duration::from_secs(2 * timing_internal_sec));
    loop {
        internal.tick().await;
        let current = timestamp();

        let time = current - current % (expire_time * 1000) as u64;

        let key = clean_up_key(time);
        if exists(key.clone())? {
            for member in smembers::<String>(key.clone())? {
                hdel(evict_key_to_time_key(), member.clone())?;
                if get(member.clone()).is_err() {
                    info!("evict expired key: {}", member);
                }
            }
        }
        delete(key)?;
    }
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CacheConfig {
    controller_addr: Option<String>,
    executor_addr: Option<String>,
    crypto_addr: Option<String>,
    redis_addr: Option<String>,
    timing_internal_sec: Option<u64>,
    timing_batch: Option<u64>,
    account: String,
    log_level: LogLevel,
    expire_time: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            controller_addr: Some("http://127.0.0.1:50004".to_string()),
            executor_addr: Some("http://127.0.0.1:50002".to_string()),
            crypto_addr: Some("http://127.0.0.1:50005".to_string()),
            redis_addr: Some("redis://default:rivtower@127.0.0.1:6379".to_string()),
            timing_internal_sec: Some(1),
            timing_batch: Some(100),
            account: "757ca1c731a3d7e9bdbd0e22ee65918674a77bd7".to_string(),
            log_level: LogLevel::Normal,
            expire_time: Some(60),
        }
    }
}
impl Display for CacheConfig {
    fn to_json(&self) -> Value {
        json!({
            "controller_addr": self.controller_addr,
            "executor_addr": self.executor_addr,
            "crypto_addr": self.crypto_addr,
            "redis_addr": self.redis_addr,
            "timing_internal_sec": self.timing_internal_sec,
            "timing_batch": self.timing_batch,
            "account": self.account,
            "log_level": self.log_level,
            "expire_time": self.expire_time,
        })
    }
}

async fn set_cache_logger(level: LevelFilter) -> Result<()> {
    set_logger(&LOGGER)?;
    set_max_level(level);
    Ok(())
}

async fn param(
    config: CacheConfig,
) -> (
    Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
    u64,
    isize,
    usize,
) {
    let ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient> = Context::new(
        config.controller_addr.unwrap_or_default(),
        config.executor_addr.unwrap_or_default(),
        config.crypto_addr.unwrap_or_default(),
        config.redis_addr.unwrap_or_default(),
    );
    (
        ctx,
        config.timing_internal_sec.unwrap_or_default(),
        config.timing_batch.unwrap_or_default() as isize,
        config.expire_time.unwrap_or_default() as usize,
    )
}

use rocket::config::Config;
use rocket::figment::providers::{Env, Format, Toml};
use rocket::figment::{Figment, Profile};
use serde_json::{json, Value};

#[rocket::main]
async fn main() {
    init_local_utc_offset();
    let figment = Figment::from(Config::debug_default())
        .merge(Toml::file(Env::var_or("ROCKET_CONFIG", "Rocket.toml")).nested())
        .merge(Env::prefixed("ROCKET_").ignore(&["PROFILE"]).global())
        .select(Profile::from_env_or(
            "ROCKET_PROFILE",
            Config::DEFAULT_PROFILE,
        ));
    let config = figment.extract::<CacheConfig>().unwrap_or_default();
    if let Err(e) = set_cache_logger(LevelFilter::from(config.log_level)).await {
        panic!("set cache logger failed: {}", e);
    }
    info!("cache config: {}", config.display());
    let (ctx, timing_internal_sec, timing_batch, expire_time) = param(config).await;
    tokio::spawn(commit_tx_schedule(
        timing_internal_sec,
        expire_time,
        timing_batch,
        ctx.controller.clone(),
    ));
    tokio::spawn(check_tx_schedule(
        timing_internal_sec,
        timing_batch,
        expire_time,
        ctx.controller.clone(),
        ctx.evm.clone(),
    ));
    tokio::spawn(check_expired_key(timing_internal_sec, expire_time));
    let rocket: Rocket<Build> = rocket(figment).attach(AdHoc::config::<CacheConfig>());

    if let Err(e) = rocket.manage(ctx).launch().await {
        error!("Whoops! Rocket didn't launch!");
        // We drop the error to get a Rocket-formatted panic.
        drop(e);
    };
}
