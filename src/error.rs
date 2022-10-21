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

use std::num::ParseIntError;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("uri isn't normalized")]
    Uri,
    #[error("parse address or data error: {0}")]
    ParseAddress(#[from] anyhow::Error),
    #[error("parse hash error")]
    ParseHash,
    #[error("parse int error: {0}")]
    ParseInt(#[from] ParseIntError),
    #[error("connect redis error: {0}")]
    Connect(#[from] r2d2::Error),
    #[error("operate redis command error: {0}")]
    Operate(#[from] r2d2_redis::redis::RedisError),
    #[error("deserialize json error: {0}")]
    Deserialize(#[from] serde_json::error::Error),
    #[error("Query {query_type} error: {detail:?}")]
    QueryCitaCloud {
        query_type: String,
        detail: anyhow::Error,
    },
    #[error("serialize toml error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("deserialize toml error: {0}")]
    TomlDe(#[from] toml::de::Error),
}
