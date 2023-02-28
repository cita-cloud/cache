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
use crate::cita_cloud::evm::EvmBehaviour;
use crate::common::crypto::sm::Hash;
use crate::common::package::Package;
use crate::common::util::hex_without_0x;
use crate::{
    config, rpc_clients, ControllerClient, CryptoClient, EvmClient, ExecutorClient, RpcClients,
};
use anyhow::Result;
use cita_cloud_proto::blockchain::raw_transaction::Tx;
use cita_cloud_proto::blockchain::RawTransaction;
use cita_cloud_proto::executor::CallRequest;
use msgpack_schema::deserialize;
use prost::Message;

#[tonic::async_trait]
pub trait Layer1Adaptor {
    fn new() -> Self
    where
        Self: Sized;

    async fn send_transaction(&self, transaction: Vec<u8>) -> Result<Hash>;

    async fn get_transaction(&self, hash: Hash) -> Result<Vec<u8>>;

    async fn get_transaction_and_try_decode(
        &self,
        hash: Hash,
        account: Vec<u8>,
    ) -> Result<Option<(String, Vec<u8>, u64)>>;

    async fn get_receipt(&self, hash: Hash) -> Result<Vec<u8>>;

    async fn estimate_fee(&self, data: Vec<u8>) -> Result<Vec<u8>>;

    async fn get_block_by_number(&self, height: u64) -> Result<Vec<u8>>;

    async fn get_block_number(&self, for_pending: bool) -> Result<u64>;

    async fn get_system_config(&self) -> Result<Vec<u8>>;
}

#[derive(Clone)]
pub struct Mock;

#[tonic::async_trait]
impl Layer1Adaptor for Mock {
    fn new() -> Self {
        Self
    }

    async fn send_transaction(&self, _transaction: Vec<u8>) -> Result<Hash> {
        Ok(Hash::default())
    }

    async fn get_transaction(&self, _hash: Hash) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn get_transaction_and_try_decode(
        &self,
        _hash: Hash,
        _account: Vec<u8>,
    ) -> Result<Option<(String, Vec<u8>, u64)>> {
        Ok(None)
    }

    async fn get_receipt(&self, _hash: Hash) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn estimate_fee(&self, _data: Vec<u8>) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn get_block_by_number(&self, _height: u64) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn get_block_number(&self, _for_pending: bool) -> Result<u64> {
        Ok(1)
    }

    async fn get_system_config(&self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

#[derive(Clone)]
pub struct CitaCloud {
    rpc_clients: RpcClients<ControllerClient, ExecutorClient, EvmClient, CryptoClient>,
}

#[tonic::async_trait]
impl Layer1Adaptor for CitaCloud {
    fn new() -> Self {
        let rpc_clients = rpc_clients();
        Self { rpc_clients }
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

    async fn get_transaction_and_try_decode(
        &self,
        hash: Hash,
        account: Vec<u8>,
    ) -> Result<Option<(String, Vec<u8>, u64)>> {
        let raw = self.rpc_clients.controller.get_tx(hash).await?;
        if let Some(Tx::NormalTx(normal_tx)) = raw.tx {
            let sender: Vec<u8> = normal_tx.witness.expect("get witness failed!").sender;
            if sender == account {
                let package_data = normal_tx.transaction.expect("get transaction failed!").data;
                let decoded_package = deserialize::<Package>(package_data.as_slice())?;
                let batch_number = decoded_package.batch_number;
                info!("poll batch: {}!", batch_number);
                let hash_str = hex_without_0x(normal_tx.transaction_hash.as_slice());
                Ok(Some((hash_str, package_data, batch_number)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
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

    async fn get_block_by_number(&self, height: u64) -> Result<Vec<u8>> {
        let block = self
            .rpc_clients
            .controller
            .get_block_by_number(height)
            .await?;
        let mut block_bytes = Vec::with_capacity(block.encoded_len());
        block.encode(&mut block_bytes)?;
        Ok(block_bytes)
    }

    async fn get_block_number(&self, for_pending: bool) -> Result<u64> {
        self.rpc_clients
            .controller
            .get_block_number(for_pending)
            .await
    }

    async fn get_system_config(&self) -> Result<Vec<u8>> {
        let config = self.rpc_clients.controller.get_system_config().await?;
        let mut config_bytes = Vec::with_capacity(config.encoded_len());
        config.encode(&mut config_bytes)?;
        Ok(config_bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u64)]
pub enum Layer1Type {
    CitaCloud = 0,
    Mock,
}

impl TryFrom<u64> for Layer1Type {
    type Error = ();

    fn try_from(v: u64) -> Result<Self, Self::Error> {
        match v {
            x if x == Layer1Type::CitaCloud as u64 => Ok(Layer1Type::CitaCloud),
            x if x == Layer1Type::Mock as u64 => Ok(Layer1Type::Mock),
            _ => Err(()),
        }
    }
}

#[derive(Clone)]
pub struct Layer1 {
    cita_cloud: CitaCloud,
    mock: Mock,
}

#[tonic::async_trait]
impl Layer1Adaptor for Layer1 {
    fn new() -> Self {
        Self {
            cita_cloud: CitaCloud::new(),
            mock: Mock::new(),
        }
    }

    async fn send_transaction(&self, transaction: Vec<u8>) -> Result<Hash> {
        let config = config();

        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => self.cita_cloud.send_transaction(transaction).await,
            Layer1Type::Mock => self.mock.send_transaction(transaction).await,
        }
    }

    async fn get_transaction(&self, hash: Hash) -> Result<Vec<u8>> {
        let config = config();
        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => self.cita_cloud.get_transaction(hash).await,
            Layer1Type::Mock => self.mock.get_transaction(hash).await,
        }
    }

    async fn get_transaction_and_try_decode(
        &self,
        hash: Hash,
        account: Vec<u8>,
    ) -> Result<Option<(String, Vec<u8>, u64)>> {
        let config = config();
        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => {
                self.cita_cloud
                    .get_transaction_and_try_decode(hash, account)
                    .await
            }
            Layer1Type::Mock => {
                self.mock
                    .get_transaction_and_try_decode(hash, account)
                    .await
            }
        }
    }

    async fn get_receipt(&self, hash: Hash) -> Result<Vec<u8>> {
        let config = config();
        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => self.cita_cloud.get_receipt(hash).await,
            Layer1Type::Mock => self.mock.get_receipt(hash).await,
        }
    }

    async fn estimate_fee(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        let config = config();
        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => self.cita_cloud.estimate_fee(data).await,
            Layer1Type::Mock => self.mock.estimate_fee(data).await,
        }
    }

    async fn get_block_by_number(&self, height: u64) -> Result<Vec<u8>> {
        let config = config();
        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => self.cita_cloud.get_block_by_number(height).await,
            Layer1Type::Mock => self.mock.get_block_by_number(height).await,
        }
    }

    async fn get_block_number(&self, for_pending: bool) -> Result<u64> {
        let config = config();
        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => self.cita_cloud.get_block_number(for_pending).await,
            Layer1Type::Mock => self.mock.get_block_number(for_pending).await,
        }
    }

    async fn get_system_config(&self) -> Result<Vec<u8>> {
        let config = config();
        match Layer1Type::try_from(config.layer1_type).expect("layer1 type invalid!") {
            Layer1Type::CitaCloud => self.cita_cloud.get_system_config().await,
            Layer1Type::Mock => self.mock.get_system_config().await,
        }
    }
}
