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

use crate::cita_cloud::controller::ControllerClient;
use crate::cita_cloud::crypto::CryptoClient;
use crate::cita_cloud::evm::EvmClient;
use crate::cita_cloud::executor::ExecutorClient;
use crate::common::cache_log::LOGGER;
use crate::common::constant::{
    CACHE_CONFIG, CONTROLLER_CLIENT, CRYPTO_CLIENT, EVM_CLIENT, EXECUTOR_CLIENT, KEY_PAIR,
    LOCAL_EVM_CLIENT, LOCAL_EXECUTOR_CLIENT, RECEIPT, ROUGH_INTERNAL, TX,
};
use crate::common::crypto::{ArrayLike, Hash};
use crate::common::display::Display;
use crate::core::context::Context;
use crate::redis::{
    con, delete, exists, get, hdel, hget, hset, incr_one, keys, pool, psubscribe, smembers, srem,
    zadd, zrange_withscores, zrem,
};
use ::log::LevelFilter;
use anyhow::Result;
use log::{set_logger, set_max_level};
use rest_api::common::{api_not_found, uri_not_found, ApiDoc};
use rest_api::get::{
    abi, account_nonce, balance, block, block_hash, block_number, code, receipt, receipt_inner,
    system_config, tx, version,
};
use rest_api::post::{call, change_role, create, send_tx};
use rocket::config::LogLevel;
use rocket::fairing::AdHoc;
use rocket::{routes, Build, Rocket};
use serde::Deserialize;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::cita_cloud::wallet::CryptoType;
use crate::common::context::{BlockContext, LocalBehaviour};
use crate::common::util::init_local_utc_offset;
use crate::core::key_manager::{CacheBehavior, CacheManager};
use crate::core::schedule_task::{
    CheckTxTask, CommitTxTask, EvictExpiredKeyTask, LazyEvictExpiredKeyTask, PackTxTask,
    PollTxsTask, ReplayTask, ScheduleTask, UsefulParamTask, XaddTask,
};
use rocket::config::Config;
use rocket::figment::providers::{Env, Format, Toml};
use rocket::figment::{Figment, Profile};
use serde_json::{json, Value};

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
                account_nonce,
                receipt,
                receipt_inner,
                system_config,
                block_hash,
                call,
                create,
                send_tx,
                change_role,
                version,
            ],
        )
        .register("/", catchers![uri_not_found])
        .register("/api", catchers![api_not_found])
}

#[derive(Deserialize, Clone, Debug)]
#[serde(crate = "rocket::serde")]
pub struct CacheConfig {
    controller_addr: Option<String>,
    executor_addr: Option<String>,
    local_executor_addr: Option<String>,
    crypto_addr: Option<String>,
    redis_addr: Option<String>,
    timing_internal_sec: Option<u64>,
    timing_batch: Option<u64>,
    redis_max_workers: Option<u64>,
    log_level: LogLevel,
    //read cache timeout
    expire_time: Option<u64>,
    //collect expired keys in rough_internal seconds
    rough_internal: Option<u64>,
    workers: u64,
    crypto_type: CryptoType,
    is_master: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            controller_addr: Some("http://127.0.0.1:50004".to_string()),
            executor_addr: Some("http://127.0.0.1:50002".to_string()),
            local_executor_addr: Some("http://127.0.0.1:55556".to_string()),
            crypto_addr: Some("http://127.0.0.1:50005".to_string()),
            redis_addr: Some("redis://default:rivtower@127.0.0.1:6379".to_string()),
            timing_internal_sec: Some(1),
            timing_batch: Some(100),
            redis_max_workers: Some(100),
            log_level: LogLevel::Normal,
            expire_time: Some(60),
            rough_internal: Some(10),
            workers: 1,
            crypto_type: CryptoType::Sm,
            is_master: true,
        }
    }
}
impl Display for CacheConfig {
    fn to_json(&self) -> Value {
        json!({
            "controller_addr": self.controller_addr,
            "executor_addr": self.executor_addr,
            "local_executor_addr": self.local_executor_addr,
            "crypto_addr": self.crypto_addr,
            "redis_addr": self.redis_addr,
            "timing_internal_sec": self.timing_internal_sec,
            "timing_batch": self.timing_batch,
            "redis_max_workers": self.redis_max_workers,
            "log_level": self.log_level,
            "expire_time": self.expire_time,
            "rough_internal": self.rough_internal,
            "workers": self.workers,
            "crypto_type": self.crypto_type,
            "is_master": self.is_master,
        })
    }
}

async fn set_cache_logger(level: LevelFilter) -> Result<()> {
    set_logger(&LOGGER)?;
    set_max_level(level);
    Ok(())
}

async fn set_param(
    config: CacheConfig,
) -> (
    Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
    u64,
    isize,
    usize,
) {
    if let Err(e) = CACHE_CONFIG.set(config.clone()) {
        panic!("store cache config error: {e:?}");
    }
    let ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient> = Context::new(
        config.controller_addr.unwrap_or_default(),
        config.executor_addr.unwrap_or_default(),
        config.local_executor_addr.unwrap_or_default(),
        config.crypto_addr,
        config.redis_addr.unwrap_or_default(),
        config.redis_max_workers.unwrap_or_default(),
    );
    if let Err(e) = CONTROLLER_CLIENT.set(ctx.controller.clone()) {
        panic!("store controller client error: {e:?}");
    }
    if let Err(e) = EXECUTOR_CLIENT.set(ctx.executor.clone()) {
        panic!("store executor client error: {e:?}");
    }
    if let Err(e) = LOCAL_EXECUTOR_CLIENT.set(ctx.local_executor.clone()) {
        panic!("store local executor client error: {e:?}");
    }
    if let Err(e) = EVM_CLIENT.set(ctx.evm.clone()) {
        panic!("store evm client error: {e:?}");
    }
    if let Err(e) = LOCAL_EVM_CLIENT.set(ctx.local_evm.clone()) {
        panic!("store evm client error: {e:?}");
    }
    if let Some(crypto) = ctx.crypto.clone() {
        if let Err(e) = CRYPTO_CLIENT.set(crypto) {
            panic!("store crypto client error: {e:?}");
        }
    }
    if let Err(e) = ROUGH_INTERNAL.set(config.rough_internal.unwrap_or_default()) {
        panic!("set rough internal fail: {e:?}")
    }
    (
        ctx,
        config.timing_internal_sec.unwrap_or_default(),
        config.timing_batch.unwrap_or_default() as isize,
        config.expire_time.unwrap_or_default() as usize,
    )
}

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
        panic!("set cache logger failed: {e}");
    }
    info!("cache config: {}", config.display());
    let (ctx, timing_internal_sec, timing_batch, expire_time) = set_param(config.clone()).await;
    let con = &mut con();
    match BlockContext::set_up(con, expire_time, config.crypto_type, config.is_master).await {
        Ok(_) => info!("block context set up success!"),
        Err(e) => warn!("block context set up fail: {}", e),
    }
    match CacheManager::set_up(con).await {
        Ok(_) => info!("cache manager set up success!"),
        Err(e) => warn!("cache manager set up fail: {}", e),
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_name("schedule-thread")
        .worker_threads(10)
        .enable_all()
        .build()
        .expect("create tokio runtime");
    runtime.spawn(PackTxTask::schedule(
        timing_internal_sec,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(CommitTxTask::schedule(
        timing_internal_sec,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(CheckTxTask::schedule(
        2 * timing_internal_sec,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(EvictExpiredKeyTask::schedule(
        timing_internal_sec * 10,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(PollTxsTask::schedule(
        timing_internal_sec,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(ReplayTask::schedule(
        timing_internal_sec,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(LazyEvictExpiredKeyTask::schedule(
        timing_internal_sec * 2,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(UsefulParamTask::schedule(
        timing_internal_sec,
        timing_batch,
        expire_time,
    ));
    runtime.spawn(XaddTask::schedule(
        timing_internal_sec,
        timing_batch,
        expire_time,
    ));

    let rocket: Rocket<Build> = rocket(figment).attach(AdHoc::config::<CacheConfig>());

    if let Err(e) = rocket.manage(ctx).launch().await {
        error!("Whoops! Rocket didn't launch!");
        // We drop the error to get a Rocket-formatted panic.
        drop(e);
    };
}
