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
use crate::core::crypto::CryptoBehaviour;
use crate::core::evm::EvmBehaviour;
use crate::core::executor::ExecutorBehaviour;
use crate::pool;
use crate::redis::Pool;
use cita_cloud_proto::client::ClientOptions;
use tokio::sync::OnceCell;

pub const CLIENT_NAME: &str = "cache";

pub struct Context<Co, Ex, Ev, Cr> {
    /// Those gRPC client are connected lazily.
    pub controller: Co,
    pub executor: Ex,
    pub evm: Ev,
    pub crypto: Cr,
    pub redis_pool: Pool,
}

impl<Co, Ex, Ev, Cr> Context<Co, Ex, Ev, Cr> {
    pub fn new(
        controller_addr: String,
        executor_addr: String,
        crypto_addr: String,
        redis_addr: String,
    ) -> Self
    where
        Co: ControllerBehaviour + Clone,
        Ex: ExecutorBehaviour + Clone,
        Ev: EvmBehaviour + Clone,
        Cr: CryptoBehaviour + Clone,
    {
        let controller_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(CLIENT_NAME.to_string(), controller_addr);
            match client_options.connect_rpc() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));
        let executor_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(CLIENT_NAME.to_string(), executor_addr.clone());
            match client_options.connect_executor() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));

        let evm_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(CLIENT_NAME.to_string(), executor_addr);
            match client_options.connect_evm() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));

        let crypto_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(CLIENT_NAME.to_string(), crypto_addr);
            match client_options.connect_crypto() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));

        let redis_pool = pool(redis_addr);
        let controller = Co::connect(controller_client);
        let executor = Ex::connect(executor_client);
        let evm = Ev::connect(evm_client);
        let crypto = Cr::connect(crypto_client);
        Self {
            controller,
            executor,
            evm,
            crypto,
            redis_pool,
        }
    }
}
