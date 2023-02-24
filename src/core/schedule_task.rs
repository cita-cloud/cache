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
use crate::redis::{Connection, Pool};
use crate::{config, LocalBehaviour};
use anyhow::Result;
use tokio::time;
// use tokio::time::MissedTickBehavior;
pub static SCHEDULE_POOL: OnceCell<Pool> = OnceCell::const_new();

pub fn get_con() -> Connection {
    SCHEDULE_POOL.get().unwrap().get()
}

#[tonic::async_trait]
pub trait ScheduleTask {
    async fn task(con: &mut Connection, timing_batch: isize, expire_time: usize) -> Result<()>;

    fn name() -> String;

    fn enable(con: &mut Connection) -> Result<bool>;

    async fn schedule(time_internal: u64, timing_batch: isize, expire_time: usize) {
        let mut internal = time::interval(time::Duration::from_millis(time_internal));
        let con = &mut get_con();
        loop {
            internal.tick().await;
            match Self::enable(con) {
                Ok(flag) => {
                    if flag {
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

    async fn schedule(time_internal: u64, timing_batch: isize, expire_time: usize) {
        let mut internal = time::interval(time::Duration::from_secs(time_internal));
        let con = &mut get_con();
        loop {
            internal.tick().await;
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

    async fn schedule(time_internal: u64, timing_batch: isize, expire_time: usize) {
        let mut internal = time::interval(time::Duration::from_secs(time_internal));
        let con = &mut get_con();
        loop {
            internal.tick().await;
            match Self::enable(con) {
                Ok(flag) => {
                    if flag {
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

pub struct LazyEvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for LazyEvictExpiredKeyTask {
    async fn task(con: &mut Connection, _: isize, _: usize) -> Result<()> {
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

use msgpack_schema::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use tokio::sync::OnceCell;

#[derive(Serialize, Deserialize, Clone)]
pub struct Enqueue {
    #[tag = 0]
    pub hash: String,
    #[tag = 1]
    pub tx: Vec<u8>,
    #[tag = 2]
    pub valid_util_block: u64,
    #[tag = 3]
    pub need_package: bool,
}

impl Enqueue {
    #[allow(dead_code)]
    pub fn new(hash: String, tx: Vec<u8>, valid_util_block: u64, need_package: bool) -> Self {
        Self {
            hash,
            tx,
            valid_util_block,
            need_package,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Expire {
    #[tag = 0]
    pub key: String,
    #[tag = 1]
    pub expire_time: u64,
}

impl Expire {
    pub fn new(key: String, expire_time: u64) -> Self {
        Self { key, expire_time }
    }
}
pub struct XaddTask;

#[tonic::async_trait]
impl ScheduleTask for XaddTask {
    async fn task(_con: &mut Connection, _: isize, _: usize) -> Result<()> {
        Ok(())
    }

    fn name() -> String {
        "xadd key".to_string()
    }

    fn enable(_con: &mut Connection) -> Result<bool> {
        Ok(true)
    }

    async fn schedule(time_internal: u64, timing_batch: isize, _expire_time: usize) {
        let con = &mut get_con();
        loop {
            if let Err(e) =
                CacheManager::sub_xadd_stream(con, time_internal, timing_batch as usize).await
            {
                warn!("[{} task] enable error: {}", Self::name(), e);
            }
        }
    }
}

pub struct ScheduleTaskManager;

impl ScheduleTaskManager {
    pub fn setup() -> Runtime {
        let config = config();
        if let Err(e) = SCHEDULE_POOL.set(Pool::new_with_workers(10u32)) {
            panic!("store schedule pool failed, {}", e);
        }
        let timing_internal_sec = config.timing_internal_sec.unwrap();
        let timing_batch = config.timing_batch.unwrap();
        let expire_time = config.expire_time.unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name("schedule-thread")
            .worker_threads(10)
            .enable_all()
            .build()
            .expect("create tokio runtime");
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
            config.stream_block_ms.unwrap(),
            config.stream_max_count.unwrap() as isize,
            expire_time,
        ));
        runtime
    }
}
