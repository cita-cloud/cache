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
use crate::redis::{con, Connection};
use crate::LocalBehaviour;
use anyhow::Result;
use tokio::time;
use tokio::time::MissedTickBehavior;

#[tonic::async_trait]
pub trait ScheduleTask {
    async fn task(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()>;

    fn name() -> String;

    fn enable(con: &mut Connection) -> Result<bool>;

    async fn schedule(time_internal: u64, timing_batch: isize, expire_time: usize) {
        let mut internal = time::interval(time::Duration::from_secs(time_internal));
        internal.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            internal.tick().await;
            // info!("[{} task] ticked!", Self::name());
            let con = &mut con();
            match Self::enable(con) {
                Ok(flag) => {
                    if flag {
                        // info!("[{} task] ticked! and enabled!", Self::name());
                        if let Err(e) = Self::task(con, timing_batch, expire_time).await {
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
    async fn task(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::commit(con, timing_batch, expire_time).await
    }

    fn name() -> String {
        "commit tx".to_string()
    }

    fn enable(con: &mut Connection) -> Result<bool> {
        BlockContext::is_master(con)
    }
}

pub struct PackTxTask;

#[tonic::async_trait]
impl ScheduleTask for PackTxTask {
    async fn task(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::package(con, timing_batch, expire_time).await
    }

    fn name() -> String {
        "pack tx".to_string()
    }

    fn enable(con: &mut Connection) -> Result<bool> {
        BlockContext::is_master(con)
    }
}

pub struct CheckTxTask;

#[tonic::async_trait]
impl ScheduleTask for CheckTxTask {
    async fn task(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::check(con, timing_batch, expire_time).await
    }

    fn name() -> String {
        "check tx".to_string()
    }

    fn enable(con: &mut Connection) -> Result<bool> {
        BlockContext::is_master(con)
    }
}

pub struct LazyEvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for LazyEvictExpiredKeyTask {
    async fn task(con: &mut Connection, _: isize, _: usize) -> Result<()> {
        CacheManager::sub_expire_event(con).await?;
        CacheManager::try_lazy_evict(con).await
    }

    fn name() -> String {
        "lazy evict expired key".to_string()
    }

    fn enable(_con: &mut Connection) -> Result<bool> {
        Ok(true)
    }
}

pub struct EvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for EvictExpiredKeyTask {
    async fn task(con: &mut Connection, _: isize, _: usize) -> Result<()> {
        CacheManager::sub_evict_event(con).await
    }

    fn name() -> String {
        "evict expired key".to_string()
    }

    fn enable(_con: &mut Connection) -> Result<bool> {
        Ok(true)
    }
}

pub struct UsefulParamTask;

#[tonic::async_trait]
impl ScheduleTask for UsefulParamTask {
    async fn task(con: &mut Connection, _: isize, expire_time: usize) -> Result<()> {
        BlockContext::timing_update(con, expire_time).await
    }

    fn name() -> String {
        "useful param task".to_string()
    }

    fn enable(_con: &mut Connection) -> Result<bool> {
        Ok(true)
    }
}

pub struct PollTxsTask;

#[tonic::async_trait]
impl ScheduleTask for PollTxsTask {
    async fn task(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::poll(con, timing_batch, expire_time).await
    }

    fn name() -> String {
        "poll txs".to_string()
    }

    fn enable(con: &mut Connection) -> Result<bool> {
        Ok(!BlockContext::is_master(con)?)
    }
}

pub struct ReplayTask;

#[tonic::async_trait]
impl ScheduleTask for ReplayTask {
    async fn task(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()> {
        CacheManager::replay(con, timing_batch, expire_time).await
    }

    fn name() -> String {
        "replay txs".to_string()
    }

    fn enable(con: &mut Connection) -> Result<bool> {
        Ok(!BlockContext::is_master(con)?)
    }
}
