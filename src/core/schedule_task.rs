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

use crate::common::context::BlockContext;
use crate::core::key_manager::{CacheBehavior, CacheManager, PackBehavior, ValidatorBehavior};
use crate::LocalBehaviour;
use anyhow::Result;
use tokio::time;

#[tonic::async_trait]
pub trait ScheduleTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()>;

    fn name() -> String;

    fn enable() -> Result<bool>;

    async fn schedule(time_internal: u64, timing_batch: isize, expire_time: usize) {
        let mut internal = time::interval(time::Duration::from_millis(time_internal));
        loop {
            match Self::enable() {
                Ok(flag) => {
                    if flag {
                        internal.tick().await;
                        if let Err(e) = Self::task(timing_batch, expire_time).await {
                            warn!("[{} task] error: {}", Self::name(), e);
                        }
                    }
                }
                Err(e) => warn!("[{} task] enable error: {}", Self::name(), e),
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

    fn enable() -> Result<bool> {
        BlockContext::is_master()
    }
}

pub struct PackTxTask;

#[tonic::async_trait]
impl ScheduleTask for PackTxTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::package(timing_batch, expire_time).await
    }

    fn name() -> String {
        "pack tx".to_string()
    }

    fn enable() -> Result<bool> {
        BlockContext::is_master()
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

    fn enable() -> Result<bool> {
        BlockContext::is_master()
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

    fn enable() -> Result<bool> {
        Ok(true)
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

    fn enable() -> Result<bool> {
        Ok(true)
    }
}

pub struct UsefulParamTask;

#[tonic::async_trait]
impl ScheduleTask for UsefulParamTask {
    async fn task(_: isize, expire_time: usize) -> Result<()> {
        BlockContext::timing_update(expire_time).await
    }

    fn name() -> String {
        "useful param task".to_string()
    }

    fn enable() -> Result<bool> {
        Ok(true)
    }
}

pub struct PollTxsTask;

#[tonic::async_trait]
impl ScheduleTask for PollTxsTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::poll(timing_batch, expire_time).await
    }

    fn name() -> String {
        "poll txs".to_string()
    }

    fn enable() -> Result<bool> {
        Ok(!BlockContext::is_master()?)
    }
}

pub struct ReplayTask;

#[tonic::async_trait]
impl ScheduleTask for ReplayTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::replay(timing_batch, expire_time).await
    }

    fn name() -> String {
        "replay txs".to_string()
    }

    fn enable() -> Result<bool> {
        Ok(!BlockContext::is_master()?)
    }
}
