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

mod api;
mod constant;
mod crypto;
mod display;
mod health_check;
mod redis;
mod util;
mod core;
mod context;
mod from_request;
mod error;

use crate::display::init_local_utc_offset;
use api::ApiDoc;
use api::{
    abi, account_nonce, api_not_found, balance, block, block_hash, block_number, code, peers_count,
    peers_info, receipt, system_config, tx, uri_not_found, version,
};
use rocket::{routes, Build, Rocket, tokio};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use crate::context::Context;
use crate::core::controller::ControllerClient;
use crate::core::evm::EvmClient;
use crate::core::executor::ExecutorClient;
use crate::redis::pool;

#[macro_use]
extern crate rocket;

fn rocket() -> Rocket<Build> {
    let ctx: Context<ControllerClient, ExecutorClient, EvmClient> = Context::new();

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
                version
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
