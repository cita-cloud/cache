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

#![allow(clippy::let_and_return)]
use anyhow::Context;
use anyhow::Result;

use prost::Message;

use crate::common::crypto::{ArrayLike, Hash};
use cita_cloud_proto::client::{InterceptedSvc, RPCClientTrait};
use cita_cloud_proto::retry::RetryClient;
use cita_cloud_proto::{
    blockchain::{
        raw_transaction::Tx, Block, CompactBlock, RawTransaction,
        Transaction as CloudNormalTransaction, UnverifiedTransaction, UnverifiedUtxoTransaction,
        UtxoTransaction as CloudUtxoTransaction, Witness,
    },
    common::{Empty, Hash as CloudHash, NodeNetInfo, NodeStatus},
    controller::rpc_service_client::RpcServiceClient,
    controller::{BlockNumber, Flag, SystemConfig},
};
use tokio::sync::OnceCell;

#[derive(Debug, Clone)]
pub struct ControllerClient {
    retry_client: OnceCell<RetryClient<RpcServiceClient<InterceptedSvc>>>,
}
#[tonic::async_trait]
pub trait ControllerBehaviour {
    fn connect(retry_client: OnceCell<RetryClient<RpcServiceClient<InterceptedSvc>>>) -> Self;

    async fn send_raw(&self, raw: RawTransaction) -> Result<Hash>;

    async fn get_system_config(&self) -> Result<SystemConfig>;

    async fn get_block_number(&self, for_pending: bool) -> Result<u64>;
    async fn get_block_hash(&self, block_number: u64) -> Result<Hash>;

    async fn get_block_by_number(&self, block_number: u64) -> Result<CompactBlock>;
    async fn get_block_by_hash(&self, hash: Hash) -> Result<CompactBlock>;

    async fn get_block_detail_by_number(&self, block_number: u64) -> Result<Block>;
    async fn get_block_detail_by_hash(&self, hash: Hash) -> Result<Block>;

    async fn get_tx(&self, tx_hash: Hash) -> Result<RawTransaction>;
    async fn get_tx_index(&self, tx_hash: Hash) -> Result<u64>;
    async fn get_tx_block_number(&self, tx_hash: Hash) -> Result<u64>;

    async fn add_node(&self, multiaddr: String) -> Result<u32>;

    async fn get_node_status(&self) -> Result<NodeStatus>;
}

#[tonic::async_trait]
impl ControllerBehaviour for ControllerClient {
    fn connect(retry_client: OnceCell<RetryClient<RpcServiceClient<InterceptedSvc>>>) -> Self {
        Self { retry_client }
    }

    async fn send_raw(&self, raw: RawTransaction) -> Result<Hash> {
        let client = self.retry_client.get().unwrap();
        let resp = client.send_raw_transaction(raw).await?;

        Hash::try_from_slice(&resp.hash)
            .context("controller returns an invalid transaction hash, maybe we are using a wrong signing algorithm?")
    }

    async fn get_system_config(&self) -> Result<SystemConfig> {
        let client = self.retry_client.get().unwrap();

        let resp = client.get_system_config(Empty {}).await?;

        Ok(resp)
    }

    async fn get_block_number(&self, for_pending: bool) -> Result<u64> {
        let client = self.retry_client.get().unwrap();

        let flag = Flag { flag: for_pending };
        let resp = client.get_block_number(flag).await?;

        Ok(resp.block_number)
    }

    async fn get_block_hash(&self, block_number: u64) -> Result<Hash> {
        let client = self.retry_client.get().unwrap();

        let block_number = BlockNumber { block_number };
        let resp = client.get_block_hash(block_number).await?;
        Hash::try_from_slice(&resp.hash)
            .context("controller returns an invalid block hash, maybe we are using a different signing algorithm?")
    }

    async fn get_block_by_number(&self, block_number: u64) -> Result<CompactBlock> {
        let client = self.retry_client.get().unwrap();

        let block_number = BlockNumber { block_number };
        let resp = client.get_block_by_number(block_number).await?;

        Ok(resp)
    }

    async fn get_block_by_hash(&self, hash: Hash) -> Result<CompactBlock> {
        let client = self.retry_client.get().unwrap();

        let hash = CloudHash {
            hash: hash.to_vec(),
        };
        let resp = client.get_block_by_hash(hash).await?;
        Ok(resp)
    }

    async fn get_block_detail_by_number(&self, block_number: u64) -> Result<Block> {
        let client = self.retry_client.get().unwrap();

        let block_number = BlockNumber { block_number };
        let resp = client.get_block_detail_by_number(block_number).await?;
        Ok(resp)
    }

    async fn get_block_detail_by_hash(&self, hash: Hash) -> Result<Block> {
        let client = self.retry_client.get().unwrap();

        let block_number = self.get_block_by_hash(hash).await?.header.unwrap().height;
        let block_number = BlockNumber { block_number };
        let resp = client.get_block_detail_by_number(block_number).await?;

        Ok(resp)
    }

    async fn get_tx(&self, tx_hash: Hash) -> Result<RawTransaction> {
        let client = self.retry_client.get().unwrap();

        let resp = client
            .get_transaction(CloudHash {
                hash: tx_hash.to_vec(),
            })
            .await?;
        Ok(resp)
    }

    async fn get_tx_index(&self, tx_hash: Hash) -> Result<u64> {
        let client = self.retry_client.get().unwrap();

        let resp = client
            .get_transaction_index(CloudHash {
                hash: tx_hash.to_vec(),
            })
            .await?;
        Ok(resp.tx_index)
    }

    async fn get_tx_block_number(&self, tx_hash: Hash) -> Result<u64> {
        let client = self.retry_client.get().unwrap();

        let resp = client
            .get_transaction_block_number(CloudHash {
                hash: tx_hash.to_vec(),
            })
            .await?;

        Ok(resp.block_number)
    }

    async fn add_node(&self, multiaddr: String) -> Result<u32> {
        let client = self.retry_client.get().unwrap();

        let node_info = NodeNetInfo {
            multi_address: multiaddr,
            ..Default::default()
        };
        let resp = client.add_node(node_info).await?;

        Ok(resp.code)
    }

    async fn get_node_status(&self) -> Result<NodeStatus> {
        let client = self.retry_client.get().unwrap();

        let resp = client.get_node_status(Empty {}).await?;

        Ok(resp)
    }
}

#[tonic::async_trait]
pub trait SignerBehaviour {
    fn hash(&self, msg: &[u8]) -> Vec<u8>;
    fn address(&self) -> &[u8];
    fn sign(&self, msg: &[u8]) -> Vec<u8>;

    async fn sign_raw_tx(&self, tx: CloudNormalTransaction) -> RawTransaction {
        // calc tx hash
        let tx_hash = {
            // build tx bytes
            let tx_bytes = {
                let mut buf = Vec::with_capacity(tx.encoded_len());
                tx.encode(&mut buf).unwrap();
                buf
            };
            self.hash(tx_bytes.as_slice())
        };
        // sign tx hash
        let sender = self.address().to_vec();
        let signature = self.sign(tx_hash.as_slice()).to_vec();

        // build raw tx
        let raw_tx = {
            let witness = Witness { sender, signature };

            let unverified_tx = UnverifiedTransaction {
                transaction: Some(tx),
                transaction_hash: tx_hash.to_vec(),
                witness: Some(witness),
            };

            RawTransaction {
                tx: Some(Tx::NormalTx(unverified_tx)),
            }
        };
        raw_tx
    }

    async fn sign_raw_utxo(&self, utxo: CloudUtxoTransaction) -> RawTransaction
    where
        Self: Sync,
    {
        // calc utxo hash
        let utxo_hash = {
            // build utxo bytes
            let utxo_bytes = {
                let mut buf = Vec::with_capacity(utxo.encoded_len());
                utxo.encode(&mut buf).unwrap();
                buf
            };
            self.hash(utxo_bytes.as_slice())
        };

        // sign utxo hash
        let sender = self.address().to_vec();
        let signature = self.sign(utxo_hash.as_slice()).to_vec();

        // build raw utxo
        let raw_utxo = {
            let witness = Witness { sender, signature };

            let unverified_utxo = UnverifiedUtxoTransaction {
                transaction: Some(utxo),
                transaction_hash: utxo_hash.to_vec(),
                witnesses: vec![witness],
            };

            RawTransaction {
                tx: Some(Tx::UtxoTx(unverified_utxo)),
            }
        };

        raw_utxo
    }
}
