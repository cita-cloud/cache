use crate::cita_cloud::controller::ControllerBehaviour;
use crate::cita_cloud::evm::EvmBehaviour;
use crate::common::display::Display;
use crate::common::util::{hex_without_0x, parse_data, timestamp};
use crate::core::key_manager::{
    clean_up_expired, clean_up_expired_by_key, clean_up_key, clean_up_tx, committed_tx_key,
    contract_pattern, hash_to_block_number, hash_to_tx, key, rough_internal, set_ex,
    uncommitted_tx_key, val_prefix,
};
use crate::zrange_withscores;
use crate::{
    delete, exists, get, hget, keys, psubscribe, smembers, zadd, zrange, zrem, ArrayLike,
    ControllerClient, EvmClient, Hash, CONTROLLER_CLIENT, EVM_CLIENT, RECEIPT, TX,
};
use anyhow::{Error, Result};
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
                warn!("{} error: {}", Self::name(), e);
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
            match Self::controller().send_raw(decoded.clone()).await {
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
                    clean_up_tx(hash.clone())?;
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

#[tonic::async_trait]
impl ScheduleTask for CheckTxTask {
    async fn task(timing_batch: isize, expire_time: usize) -> Result<()> {
        let members = zrange::<String>(committed_tx_key(), 0, timing_batch)?;
        for tx_hash in members {
            let hash = Hash::try_from_slice(&parse_data(tx_hash.clone().as_str()).unwrap()[..])?;

            match Self::evm().get_receipt(hash).await {
                Ok(receipt) => {
                    set_ex(
                        key(RECEIPT.to_string(), tx_hash.clone()),
                        receipt.display(),
                        expire_time,
                    )?;
                    info!("get tx receipt and save success, hash: {}", tx_hash.clone());
                    match Self::controller().get_tx(hash).await {
                        Ok(tx) => {
                            set_ex(
                                key(TX.to_string(), tx_hash.clone()),
                                tx.display(),
                                expire_time,
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
                    let decoded: RawTransaction =
                        Message::decode(parse_data(tx.as_str())?.as_slice())?;
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
                    clean_up_tx(tx_hash.clone())?;
                    continue;
                }
                Err(e) => {
                    set_ex(
                        key(RECEIPT.to_string(), tx_hash.clone()),
                        format!("{}", e),
                        expire_time * 5,
                    )?;
                    info!("retry -> get receipt, hash: {}", tx_hash.clone());
                }
            }
            let current = Self::controller().get_block_number(false).await?;
            let config = Self::controller().get_system_config().await?;
            let valid_until_block = hget::<u64>(hash_to_block_number(), tx_hash.clone())?;
            if valid_until_block <= current
                || valid_until_block > (current + config.block_limit as u64)
            {
                warn!("retry -> get receipt, timeout hash: {}", tx_hash.clone());
                set_ex(
                    key(RECEIPT.to_string(), tx_hash.clone()),
                    "timeout".to_string(),
                    expire_time,
                )?;
                clean_up_tx(tx_hash)?
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

        let time = current - current % (rough_internal() * 1000) as u64;

        let key = clean_up_key(time);
        if exists(key.clone())? {
            for member in smembers::<String>(key.clone())? {
                if get(member.clone()).is_err() {
                    info!("[{}]: {}", Self::name(), member);
                }
                clean_up_expired(key.clone(), member.clone())?;
            }
        }
        Ok(())
    }

    fn name() -> String {
        "lazy evict expired key".to_string()
    }
}

pub struct EvictExpiredKeyTask;

#[tonic::async_trait]
impl ScheduleTask for EvictExpiredKeyTask {
    async fn task(_: isize, _: usize) -> Result<()> {
        Ok(())
    }

    async fn schedule(_: u64, _: isize, _: usize) {
        if let Err(e) = psubscribe("__keyevent@*__:expired".to_string(), |msg| {
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
            match clean_up_expired_by_key(expired_key) {
                Ok(expired_key) => info!("evict expired key: {}", expired_key),
                Err(e) => warn!("evict expired failed: {}", e),
            }
            ControlFlow::Continue
        }) {
            warn!("subscribe channel failed: {}", e);
        }
    }

    fn name() -> String {
        "evict expired key".to_string()
    }
}
