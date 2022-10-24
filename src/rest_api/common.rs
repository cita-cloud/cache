use crate::constant::{FAILURE, SUCCESS, SUCCESS_MESSAGE};
use crate::error::CacheError;
use crate::rest_api::get::*;
use crate::rest_api::post::*;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
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
pub struct QueryResult<T> {
    pub status: u64,
    pub data: Option<T>,
    pub message: String,
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

pub fn success<T>(val: T) -> Json<QueryResult<T>> {
    Json(QueryResult {
        status: SUCCESS,
        data: Some(val),
        message: SUCCESS_MESSAGE.to_string(),
    })
}

pub fn fail<T>(e: CacheError) -> Json<QueryResult<T>> {
    Json(QueryResult {
        status: FAILURE,
        data: None,
        message: format!("{}", e),
    })
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
