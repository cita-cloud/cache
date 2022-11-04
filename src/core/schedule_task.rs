use crate::cita_cloud::controller::ControllerBehaviour;
use crate::cita_cloud::evm::EvmBehaviour;
use crate::common::constant::rough_internal;
use crate::common::display::Display;
use crate::common::util::{hex_without_0x, parse_data, timestamp};
use crate::core::key_manager::{val_prefix, CacheManager};
use crate::core::key_manager::{CacheBehavior, ContractBehavior, TxBehavior};
use crate::{
    psubscribe, ArrayLike, ControllerClient, EvmClient, Hash, CONTROLLER_CLIENT, EVM_CLIENT,
};
use anyhow::Result;
use cita_cloud_proto::blockchain::{raw_transaction::Tx, RawTransaction};
use prost::Message;
use r2d2_redis::redis::ControlFlow;
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
                warn!("[{}] error: {}", Self::name(), e);
            }
        }
    }

    fn controller() -> ControllerClient {
        CONTROLLER_CLIENT.get().unwrap().clone()
    }

    fn evm() -> EvmClient {
        EVM_CLIENT.get().unwrap().clone()
    }
}

pub struct CommitTxTask;

#[tonic::async_trait]
impl ScheduleTask for CommitTxTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        let members = CacheManager::uncommitted_txs(timing_batch)?;
        for (tx_hash, score) in members {
            let tx = CacheManager::original_tx(tx_hash.clone())?;
            let decoded: RawTransaction = Message::decode(&parse_data(tx.as_str())?[..])?;
            match Self::controller().send_raw(decoded.clone()).await {
                Ok(data) => {
                    CacheManager::commit(tx_hash.clone(), score)?;
                    let hash_str = hex_without_0x(&data);
                    info!("commit tx success, hash: {}", hash_str);
                }
                Err(e) => {
                    let empty = Vec::new();
                    let hash = if let Some(Tx::NormalTx(ref normal_tx)) = decoded.tx {
                        &normal_tx.transaction_hash
                    } else {
                        empty.as_slice()
                    };
                    let hash = hex_without_0x(hash).to_string();
                    CacheManager::save_error(hash.clone(), format!("{}", e), expire_time * 5)?;
                    CacheManager::clean_up_tx(hash.clone())?;
                    warn!("commit tx fail, hash: {}", hash);
                }
            }
        }
        Ok(())
    }

    fn name() -> String {
        "commit tx".to_string()
    }
}

pub struct CheckTxTask;

impl CheckTxTask {
    async fn check_if_timeout(tx_hash: String) -> Result<bool> {
        let current = Self::controller().get_block_number(false).await?;
        let config = Self::controller().get_system_config().await?;
        let valid_until_block = CacheManager::valid_until_block(tx_hash)?;
        Ok(valid_until_block <= current
            || valid_until_block > (current + config.block_limit as u64))
    }
}

#[tonic::async_trait]
impl ScheduleTask for CheckTxTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        let members = CacheManager::committed_txs(timing_batch)?;
        for (tx_hash, _) in members {
            let hash = Hash::try_from_slice(&parse_data(tx_hash.clone().as_str())?[..])?;
            let (receipt, expire_time, is_ok) = match Self::evm().get_receipt(hash).await {
                Ok(receipt) => {
                    info!("get tx receipt success, hash: {}", tx_hash.clone());
                    (receipt.display(), expire_time, true)
                }
                Err(e) => {
                    info!("retry -> get receipt, hash: {}", tx_hash.clone());
                    (format!("{}", e), expire_time * 5, false)
                }
            };
            CacheManager::save_receipt(tx_hash.clone(), receipt, expire_time)?;
            if is_ok {
                let (tx, expire_time) = match Self::controller().get_tx(hash).await {
                    Ok(tx) => {
                        info!("get tx success, hash: {}", tx_hash.clone());
                        (tx.display(), expire_time)
                    }
                    Err(e) => {
                        info!("retry -> get tx, hash: {}", tx_hash.clone());
                        (format!("{}", e), expire_time * 5)
                    }
                };
                CacheManager::save_tx(tx_hash.clone(), tx, expire_time)?;
                CacheManager::try_clean_contract_data(tx_hash.clone())?;
                CacheManager::clean_up_tx(tx_hash.clone())?;
                continue;
            }
            if Self::check_if_timeout(tx_hash.clone()).await? {
                warn!("retry -> get receipt, timeout hash: {}", tx_hash.clone());
                CacheManager::save_error(tx_hash.clone(), "timeout".to_string(), expire_time * 5)?;
                CacheManager::clean_up_tx(tx_hash)?
            }
        }
        Ok(())
    }

    fn name() -> String {
        "check tx".to_string()
    }
}

pub struct LazyEvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for LazyEvictExpiredKeyTask {
    async fn task(_: isize, _: usize) -> Result<()> {
        let current = timestamp();
        let time = current - current % rough_internal() as u64;
        CacheManager::try_lazy_evict(time)
    }

    fn name() -> String {
        "lazy evict expired key".to_string()
    }
}

pub struct EvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for EvictExpiredKeyTask {
    async fn task(_: isize, _: usize) -> Result<()> {
        psubscribe("__keyevent@*__:expired".to_string(), |msg| {
            let expired_key = match msg.get_payload::<String>() {
                Ok(key) => {
                    if key.starts_with(&val_prefix()) {
                        key
                    } else {
                        return ControlFlow::Continue;
                    }
                }
                Err(e) => {
                    warn!("subscribe msg has none payload: {}", e);
                    return ControlFlow::Continue;
                }
            };
            match CacheManager::clean_up_expired_by_key(expired_key) {
                Ok(expired_key) => info!("evict expired key: {}", expired_key),
                Err(e) => warn!("evict expired failed: {}", e),
            }
            ControlFlow::Continue
        })?;
        Ok(())
    }

    fn name() -> String {
        "evict expired key".to_string()
    }
}
