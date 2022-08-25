mod api;
mod constant;
mod crypto;
mod display;

use crate::display::init_local_utc_offset;
use api::ApiDoc;
use api::{
    abi, account_nonce, api_not_found, balance, block, block_hash, block_number, code, peers_count,
    peers_info, receipt, system_config, tx, uri_not_found, version,
};
use rocket::{routes, Build, Rocket};
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
                version
            ],
        )
        .register("/", catchers![uri_not_found])
        .register("/api", catchers![api_not_found])
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
