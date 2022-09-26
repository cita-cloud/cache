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

use crate::core::controller::ControllerBehaviour;
use crate::core::evm::EvmBehaviour;
use crate::core::executor::ExecutorBehaviour;
use crate::pool;
use crate::redis::Pool;
use cita_cloud_proto::client::ClientOptions;
use r2d2_redis::RedisConnectionManager;
use tokio::sync::OnceCell;

pub const CLIENT_NAME: &str = "cache";

pub struct Context<Co, Ex, Ev> {
    /// Those gRPC client are connected lazily.
    pub controller: Co,
    pub executor: Ex,
    pub evm: Ev,
    pub redis_pool: Pool,
}

impl<Co, Ex, Ev> Context<Co, Ex, Ev> {
    pub fn new() -> Self
    where
        Co: ControllerBehaviour + Clone,
        Ex: ExecutorBehaviour + Clone,
        Ev: EvmBehaviour + Clone,
    {
        let controller_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(
                CLIENT_NAME.to_string(),
                format!("http://127.0.0.1:{}", 50004),
            );
            match client_options.connect_rpc() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));
        let executor_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(
                CLIENT_NAME.to_string(),
                format!("http://127.0.0.1:{}", 50002),
            );
            match client_options.connect_executor() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));

        let evm_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(
                CLIENT_NAME.to_string(),
                format!("http://127.0.0.1:{}", 50002),
            );
            match client_options.connect_evm() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));

        let controller = Co::connect(controller_client);
        let executor = Ex::connect(executor_client);
        let evm = Ev::connect(evm_client);
        let redis_pool = pool();
        Self {
            controller,
            executor,
            evm,
            redis_pool,
        }
    }

    pub fn get_redis_connection(&self) -> r2d2::PooledConnection<RedisConnectionManager> {
        self.redis_pool.get().unwrap()
    }
}
