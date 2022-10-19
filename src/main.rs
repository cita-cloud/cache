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
mod from_data;
mod from_request;
mod health_check;
mod redis;
mod rest_api;
mod util;

use crate::context::Context;
use crate::core::controller::ControllerClient;
use crate::core::evm::EvmClient;
use crate::core::executor::ExecutorClient;
use crate::display::init_local_utc_offset;
use crate::redis::pool;
use rest_api::common::{api_not_found, uri_not_found, ApiDoc};
use rest_api::get::{
    abi, account_nonce, balance, block, block_hash, block_number, code, peers_count, peers_info,
    receipt, system_config, tx, version,
};
use rest_api::post::{create, generate_account, send_tx};
use rocket::{routes, Build, Rocket};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use crate::core::crypto::CryptoClient;

#[macro_use]
extern crate rocket;

fn rocket() -> Rocket<Build> {
    let ctx: Context<ControllerClient, ExecutorClient, EvmClient, CryptoClient> = Context::new();

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
                generate_account,
                send_tx,
            ],
        )
        .register("/", catchers![uri_not_found])
        .register("/api", catchers![api_not_found])
        .manage(ctx)
}

#[rocket::main]
async fn main() {
    init_local_utc_offset();

    if let Err(e) = rocket().launch().await {
        println!("Whoops! Rocket didn't launch!");
        // We drop the error to get a Rocket-formatted panic.
        drop(e);
    };
}
