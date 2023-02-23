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
use crate::config;
use cita_cloud_proto::client::ClientOptions;
use tokio::sync::OnceCell;

pub const CLIENT_NAME: &str = "cache";

#[derive(Clone)]
pub struct RpcClients<Co, Ex, Ev, Cr> {
    /// Those gRPC client are connected lazily.
    pub controller: Co,
    pub executor: Ex,
    pub local_executor: Ex,
    pub evm: Ev,
    pub local_evm: Ev,
    pub crypto: Option<Cr>,
}

impl<Co: Clone, Ex: Clone, Ev: Clone, Cr: Clone> RpcClients<Co, Ex, Ev, Cr> {
    fn client_options(addr: String) -> ClientOptions {
        ClientOptions::new(CLIENT_NAME.to_string(), addr)
    }
    pub fn new() -> Self
    where
        Co: ControllerBehaviour + Clone,
        Ex: ExecutorBehaviour + Clone,
        Ev: EvmBehaviour + Clone,
        Cr: CryptoBehaviour + Clone,
    {
        let config = config();
        let (controller_addr, executor_addr, local_executor_addr, crypto_addr) = (
            config.controller_addr.unwrap_or_default(),
            config.executor_addr.unwrap_or_default(),
            config.local_executor_addr.unwrap_or_default(),
            config.crypto_addr,
        );
        let mut crypto = None;
        if let Some(crypto_addr) = crypto_addr {
            let crypto_client = OnceCell::new_with(Some({
                match Self::client_options(crypto_addr).connect_crypto() {
                    Ok(retry_client) => retry_client,
                    Err(e) => panic!("client init error: {:?}", &e),
                }
            }));
            crypto = Some(Cr::connect(crypto_client));
        }

        let controller = Co::connect(OnceCell::new_with(Some({
            match Self::client_options(controller_addr).connect_rpc() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        })));
        let executor = Ex::connect(OnceCell::new_with(Some({
            match Self::client_options(executor_addr.clone()).connect_executor() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        })));
        let local_executor = Ex::connect(OnceCell::new_with(Some({
            match Self::client_options(local_executor_addr.clone()).connect_executor() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        })));
        let evm = Ev::connect(OnceCell::new_with(Some({
            match Self::client_options(executor_addr).connect_evm() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        })));
        let local_evm = Ev::connect(OnceCell::new_with(Some({
            match Self::client_options(local_executor_addr).connect_evm() {
                Ok(retry_client) => retry_client,
                Err(e) => panic!("client init error: {:?}", &e),
            }
        })));

        Self {
            controller,
            executor,
            local_executor,
            evm,
            local_evm,
            crypto,
        }
    }
}
