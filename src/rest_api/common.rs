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

use crate::common::constant::{FAILURE, SUCCESS, SUCCESS_MESSAGE};
use crate::rest_api::get::*;
use crate::rest_api::post::*;
use anyhow::Error;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{OpenApi, ToSchema};

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

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct SuccessResult {
    #[schema(example = 1)]
    pub status: u64,
    #[schema(example = "success")]
    pub message: String,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct FailureResult {
    #[schema(example = 0)]
    pub status: u64,
    #[schema(example = "error message")]
    pub message: String,
}

#[derive(OpenApi)]
#[openapi(
paths(
block_number,
abi,
balance,
block,
code,
tx,
// peers_count,
// peers_info,
account_nonce,
receipt,
system_config,
block_hash,
// version,
call,
create,
send_tx,
),
components(
schemas(SuccessResult, FailureResult, CreateContract<'_>, SendTx<'_>, Call<'_>)
)
)]
pub struct ApiDoc;
