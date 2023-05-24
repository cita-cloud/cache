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

use opentelemetry::global;
use rocket::request::FromRequest;
use rocket::{request, Request};
use std::collections::HashMap;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct CtxMap(pub HashMap<String, String>);

use tracing::instrument;
#[rocket::async_trait]
impl<'r> FromRequest<'r> for CtxMap {
    type Error = ();

    #[instrument(skip_all)]
    async fn from_request(_: &'r Request<'_>) -> request::Outcome<Self, ()> {
        let mut map = HashMap::new();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&tracing::Span::current().context(), &mut map)
        });
        request::Outcome::Success(CtxMap(map))
    }
}
