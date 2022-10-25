use crate::constant::{FAILURE, SUCCESS, SUCCESS_MESSAGE};
use crate::rest_api::get::*;
use crate::rest_api::post::*;
use anyhow::Error;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use serde_json::Value;
use utoipa::Component;
use utoipa::OpenApi;

#[catch(404)]
pub fn uri_not_found() -> Json<String> {
    Json(String::from("URI not found"))
}

#[catch(404)]
pub fn api_not_found() -> Json<String> {
    Json(String::from("api not found"))
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
pub struct CacheResult<T> {
    pub status: u64,
    pub data: Option<T>,
    pub message: String,
}

pub fn success<T>(data: T) -> CacheResult<T> {
    CacheResult {
        status: SUCCESS,
        data: Some(data),
        message: SUCCESS_MESSAGE.to_string(),
    }
}

pub fn failure(e: Error) -> CacheResult<Value> {
    CacheResult {
        status: FAILURE,
        data: None,
        message: format!("{}", e),
    }
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
#[component(example = json ! ({"status": 1, "message": "success"}))]
pub struct SuccessResult {
    pub status: u64,
    pub message: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
#[derive(Component)]
#[component(example = json ! ({"status": 0, "message": "error message"}))]
pub struct FailureResult {
    pub status: u64,
    pub message: String,
}

#[derive(OpenApi)]
#[openapi(
handlers(
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
),
components(SuccessResult, FailureResult, CreateContract<'_>, SendTx<'_>)
)]
pub struct ApiDoc;
