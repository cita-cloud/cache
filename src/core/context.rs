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

use crate::cita_cloud::controller::ControllerBehaviour;
use crate::cita_cloud::crypto::CryptoBehaviour;
use crate::cita_cloud::evm::EvmBehaviour;
use crate::cita_cloud::executor::ExecutorBehaviour;
use crate::common::constant::REDIS_POOL;
use crate::pool;
use crate::redis::Pool;
use cita_cloud_proto::client::ClientOptions;
use tokio::sync::OnceCell;

pub const CLIENT_NAME: &str = "cache";

#[derive(Clone)]
pub struct Context<Co, Ex, Ev, Cr> {
    /// Those gRPC client are connected lazily.
    pub controller: Co,
    pub executor: Ex,
    pub local_executor: Ex,
    pub evm: Ev,
    pub local_evm: Ev,
    pub crypto: Option<Cr>,
    pub redis_pool: Pool,
}

impl<Co: Clone, Ex: Clone, Ev: Clone, Cr: Clone> Context<Co, Ex, Ev, Cr> {
    pub fn new(
        controller_addr: String,
        executor_addr: String,
        local_executor_addr: String,
        crypto_addr: Option<String>,
        redis_addr: String,
        workers: u64,
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
        let local_executor_client = OnceCell::new_with(Some({
            let client_options =
                ClientOptions::new(CLIENT_NAME.to_string(), local_executor_addr.clone());
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

        let local_evm_client = OnceCell::new_with(Some({
            let client_options = ClientOptions::new(CLIENT_NAME.to_string(), local_executor_addr);
            match client_options.connect_evm() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        }));

        let mut crypto = None;
        if let Some(crypto_addr) = crypto_addr {
            let crypto_client = OnceCell::new_with(Some({
                let client_options = ClientOptions::new(CLIENT_NAME.to_string(), crypto_addr);
                match client_options.connect_crypto() {
                    Ok(retry_client) => retry_client,
                    Err(e) => panic!("client init error: {:?}", &e),
                }
            }));
            crypto = Some(Cr::connect(crypto_client));
        }

        let redis_pool = pool(redis_addr, workers as u32);
        if let Err(e) = REDIS_POOL.set(redis_pool.clone()) {
            error!("set redis pool fail: {:?}", e)
        };
        let controller = Co::connect(controller_client);
        let executor = Ex::connect(executor_client);
        let local_executor = Ex::connect(local_executor_client);
        let evm = Ev::connect(evm_client);
        let local_evm = Ev::connect(local_evm_client);

        Self {
            controller,
            executor,
            local_executor,
            evm,
            local_evm,
            crypto,
            redis_pool,
        }
    }
}
