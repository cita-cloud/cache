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
use anyhow::Result;
use cita_cloud_proto::blockchain::RawTransaction;
use cita_cloud_proto::executor::CallRequest;
use crate::common::crypto::sm::Hash;
use crate::{ControllerClient, CryptoClient, EvmClient, ExecutorClient, rpc_clients, RpcClients};
use crate::cita_cloud::controller::ControllerBehaviour;
use prost::Message;
use crate::cita_cloud::evm::EvmBehaviour;

#[tonic::async_trait]
pub trait Layer1Adaptor {
    fn new() -> Self where Self: Sized;

    async fn send_transaction(&self, transaction: Vec<u8>) -> Result<Hash>;

    async fn get_transaction(&self, hash: Hash) -> Result<Vec<u8>>;

    async fn get_receipt(&self, hash: Hash) -> Result<Vec<u8>>;

    async fn estimate_fee(&self, data: Vec<u8>) -> Result<Vec<u8>>;

}

#[derive(Clone)]
pub struct Local;

#[tonic::async_trait]
impl Layer1Adaptor for Local {
    fn new() -> Self {
        Self
    }

    async fn send_transaction(&self, transaction: Vec<u8>) -> Result<Hash> {
        Ok(Hash::default())
    }

    async fn get_transaction(&self, hash: Hash) -> Result<Vec<u8>> {
       Ok(Vec::new())
    }

    async fn get_receipt(&self, hash: Hash) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn estimate_fee(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

#[derive(Clone)]
pub struct CitaCloud {
    rpc_clients: RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>
}

#[tonic::async_trait]
impl Layer1Adaptor for CitaCloud {
    fn new() -> Self {
        let rpc_clients = rpc_clients();
        Self {
            rpc_clients
        }
    }

    async fn send_transaction(&self, transaction: Vec<u8>) -> Result<Hash> {
        let tx: RawTransaction = Message::decode(transaction.as_slice())?;
        self.rpc_clients.controller.send_raw(tx).await
    }

    async fn get_transaction(&self, hash: Hash) -> Result<Vec<u8>> {
        let tx = self.rpc_clients.controller.get_tx(hash).await?;
        let mut tx_bytes = Vec::with_capacity(tx.encoded_len());
        tx.encode(&mut tx_bytes)?;
        Ok(tx_bytes)
    }

    async fn get_receipt(&self, hash: Hash) -> Result<Vec<u8>> {
        let receipt = self.rpc_clients.evm.get_receipt(hash).await?;
        let mut receipt_bytes = Vec::with_capacity(receipt.encoded_len());
        receipt.encode(&mut receipt_bytes)?;
        Ok(receipt_bytes)
    }

    async fn estimate_fee(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        let req: CallRequest = Message::decode(data.as_slice())?;
        Ok(self.rpc_clients.evm.estimate_quota(req).await?.bytes_quota)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer1Type {
    CitaCloud,
    Local
}

pub struct Facade {
    adaptor: Box<dyn Layer1Adaptor>
}

impl Facade {
    pub fn from(l1_type: Layer1Type) -> Box<dyn Layer1Adaptor> {
        match l1_type {
            Layer1Type::CitaCloud => Box::new(CitaCloud::new()),
            Layer1Type::Local => Box::new(Local::new()),
        }
    }
}