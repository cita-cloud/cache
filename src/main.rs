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

use crate::context::Context;
use crate::core::account::Account;
use crate::core::controller::{ControllerBehaviour, ControllerClient, TransactionSenderBehaviour};
use crate::core::crypto::CryptoClient;
use crate::core::evm::{EvmBehaviour, EvmClient};
use crate::core::executor::ExecutorClient;
use crate::crypto::{ArrayLike, Hash};
use crate::display::init_local_utc_offset;
use crate::display::Display;
use crate::redis::{hdel, hget, hset, pool, set, zadd, zrange, zrange_withscores, zrem};
use crate::rest_api::post::new_raw_tx;
use crate::util::{
    committed_tx_key, hash_to_receipt, hash_to_tx, hex_without_0x, key, parse_data,
    uncommitted_tx_key,
};
use anyhow::Result;
use cita_cloud_proto::blockchain::{raw_transaction::Tx, RawTransaction};
use prost::Message;
use rest_api::common::{api_not_found, uri_not_found, ApiDoc};
use rest_api::get::{
    abi, account_nonce, balance, block, block_hash, block_number, code, peers_count, peers_info,
    receipt, system_config, tx, version,
};
use rest_api::post::{create, send_tx};
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

async fn process(
    timing_batch: isize,
    controller: ControllerClient,
    evm: EvmClient,
    crypto: CryptoClient,
) -> Result<()> {
    let members = zrange_withscores::<String>(uncommitted_tx_key(), 0, timing_batch)?;
    for (fake_tx_hash, score) in members {
        let tx = hget(hash_to_tx(), fake_tx_hash.clone())?;
        let decoded: RawTransaction = Message::decode(&parse_data(tx.as_str()).unwrap()[..])?;
        println!("{}", decoded.display());
        match controller.send_raw(decoded.clone()).await {
            Ok(data) => {
                zrem(uncommitted_tx_key(), fake_tx_hash.clone())?;
                zadd(committed_tx_key(), fake_tx_hash.clone(), score)?;
                let hash_str = hex_without_0x(&data);
                hset(hash_to_receipt(), fake_tx_hash, hash_str.clone())?;
                println!("send raw tx success: {}", hash_str.clone());
            }
            Err(e) => {
                let err_str = format!("{}", e);
                if err_str.contains("InvalidValidUntilBlock") {
                    zrem(uncommitted_tx_key(), fake_tx_hash.clone())?;
                    if let Some(Tx::NormalTx(normal_tx)) = decoded.tx {
                        let raw_tx =
                            new_raw_tx(controller.clone(), normal_tx.transaction.unwrap()).await?;
                        let data = controller
                            .send_raw_tx(&Account::new(crypto.clone()), raw_tx.clone())
                            .await?;
                        println!("recommit tx, fake hash: {}", data.display());
                        continue;
                    }
                }
                let empty = Vec::new();
                let hash = match decoded.tx {
                    Some(Tx::NormalTx(ref normal_tx)) => &normal_tx.transaction_hash,
                    Some(Tx::UtxoTx(ref utxo_tx)) => &utxo_tx.transaction_hash,
                    None => empty.as_slice(),
                };
                let hash_str = hex_without_0x(hash);
                hset(hash_to_receipt(), fake_tx_hash, hash_str.clone())?;
                set(key("receipt", &hash_str), err_str)?;
            }
        }
    }
    let members = zrange::<String>(committed_tx_key(), 0, timing_batch)?;
    for fake_tx_hash in members {
        let receipt = hget(hash_to_receipt(), fake_tx_hash.clone())?;
        let hash_str = receipt.as_str();
        match evm
            .get_receipt(Hash::try_from_slice(&parse_data(hash_str).unwrap()[..]).unwrap())
            .await
        {
            Ok(receipt) => {
                set(key("receipt", hash_str), receipt.display())?;
                zrem(committed_tx_key(), fake_tx_hash.clone())?;
                zrem(uncommitted_tx_key(), fake_tx_hash.clone())?;
                hdel(hash_to_tx(), fake_tx_hash.clone())?;
                // hdel(hash_to_receipt(), fake_tx_hash.clone()).unwrap();
            }
            Err(e) => println!("get receipt fail: {:?}", e),
        }
    }
    Ok(())
}

async fn send_tx_async(
    timing_internal_sec: u64,
    timing_batch: isize,
    controller: ControllerClient,
    evm: EvmClient,
    crypto: CryptoClient,
) -> Result<()> {
    let mut internal = time::interval(time::Duration::from_secs(timing_internal_sec));
    loop {
        internal.tick().await;
        match process(
            timing_batch,
            controller.clone(),
            evm.clone(),
            crypto.clone(),
        )
        .await
        {
            Ok(_) => {}
            Err(e) => println!("process failed: {}", e),
        }
    }
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Config {
    controller_addr: Option<String>,
    executor_addr: Option<String>,
    crypto_addr: Option<String>,
    redis_addr: Option<String>,
    timing_internal_sec: Option<u64>,
    timing_batch: Option<u64>,
}

#[rocket::main]
async fn main() {
    init_local_utc_offset();
    let rocket: Rocket<Build> = rocket();
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

    tokio::spawn(send_tx_async(
        config.timing_internal_sec.unwrap_or(1),
        config.timing_batch.unwrap_or(100) as isize,
        ctx.controller.clone(),
        ctx.evm.clone(),
        ctx.crypto.clone(),
    ));
    if let Err(e) = rocket.manage(ctx).launch().await {
        println!("Whoops! Rocket didn't launch!");
        // We drop the error to get a Rocket-formatted panic.
        drop(e);
    };
}
