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

use crate::cita_cloud::controller::ControllerBehaviour;
use crate::cita_cloud::wallet::{Account, MaybeLocked, MultiCryptoAccount};
use crate::common::constant::controller;
use crate::common::crypto::SmCrypto;
use crate::core::key_manager::{
    key_without_param, CacheBehavior, CacheManager, ExpiredBehavior, ValBehavior,
};
use anyhow::Result;
use tokio::time;

#[tonic::async_trait]
pub trait ScheduleTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()>;

    fn name() -> String;

    async fn schedule(time_internal: u64, timing_batch: isize, expire_time: usize) {
        let mut internal = time::interval(time::Duration::from_secs(time_internal));
        loop {
            internal.tick().await;
            if let Err(e) = Self::task(timing_batch, expire_time).await {
                warn!("[{} task] error: {}", Self::name(), e);
            }
        }
    }
}

pub struct CommitTxTask;

#[tonic::async_trait]
impl ScheduleTask for CommitTxTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::commit(timing_batch, expire_time).await
    }

    fn name() -> String {
        "commit tx".to_string()
    }
}

pub struct CheckTxTask;

#[tonic::async_trait]
impl ScheduleTask for CheckTxTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::check(timing_batch, expire_time).await
    }

    fn name() -> String {
        "check tx".to_string()
    }
}

pub struct LazyEvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for LazyEvictExpiredKeyTask {
    async fn task(_: isize, _: usize) -> Result<()> {
        CacheManager::try_lazy_evict().await
    }

    fn name() -> String {
        "lazy evict expired key".to_string()
    }
}

pub struct EvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for EvictExpiredKeyTask {
    async fn task(_: isize, _: usize) -> Result<()> {
        CacheManager::sub_evict_event().await
    }

    fn name() -> String {
        "evict expired key".to_string()
    }
}
use crate::common::display::Display;
use crate::redis::set_ex;

pub struct BlockNumberTask;

#[tonic::async_trait]
impl ScheduleTask for BlockNumberTask {
    async fn task(_: isize, expire_time: usize) -> Result<()> {
        CacheManager::set_ex(
            key_without_param("block-number".to_string()),
            controller().get_block_number(false).await?,
            expire_time * 2,
        )?;
        CacheManager::set_ex(
            key_without_param("system-config".to_string()),
            controller().get_system_config().await?.display(),
            expire_time * 2,
        )?;
        let key = key_without_param("admin-account".to_string());
        if CacheManager::exist_val(key.clone())? {
            CacheManager::update_expire(key, expire_time * 2)?;
        } else {
            let account: MultiCryptoAccount = Account::<SmCrypto>::generate().into();
            let maybe_locked: MaybeLocked = account.into();
            let result = toml::to_string_pretty(&maybe_locked)?;
            CacheManager::create_expire(key.clone(), expire_time * 2)?;
            set_ex(key, result, expire_time * 2)?;
        }

        Ok(())
    }

    fn name() -> String {
        "evict expired key".to_string()
    }
}
