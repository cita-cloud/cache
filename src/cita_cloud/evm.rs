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

use anyhow::Context as _;
use anyhow::Result;

// use super::controller::{SignerBehaviour, TransactionSenderBehaviour};
use crate::common::crypto::{Address, ArrayLike, Hash};
use cita_cloud_proto::client::{EVMClientTrait, InterceptedSvc};
use cita_cloud_proto::retry::RetryClient;
use cita_cloud_proto::{
    common::{Address as CloudAddress, Hash as CloudHash},
    evm::{
        rpc_service_client::RpcServiceClient, Balance, ByteAbi, ByteCode, ByteQuota, Nonce, Receipt,
    },
    executor::CallRequest,
};
use tokio::sync::OnceCell;

// TODO: use constant array for these constant to avoid runtime parsing.

#[allow(unused)]
pub mod constant {
    /// Store action target address
    pub const STORE_ADDRESS: &str = "0xffffffffffffffffffffffffffffffffff010000";
    /// StoreAbi action target address
    pub const ABI_ADDRESS: &str = "0xffffffffffffffffffffffffffffffffff010001";
    /// Amend action target address
    pub const AMEND_ADDRESS: &str = "0xffffffffffffffffffffffffffffffffff010002";

    /// amend the abi data
    pub const AMEND_ABI: &str = "0x01";
    /// amend the account code
    pub const AMEND_CODE: &str = "0x02";
    /// amend the kv of db
    pub const AMEND_KV_H256: &str = "0x03";
    /// amend account balance
    pub const AMEND_BALANCE: &str = "0x05";
}

#[derive(Debug, Clone)]
pub struct EvmClient {
    retry_client: OnceCell<RetryClient<RpcServiceClient<InterceptedSvc>>>,
}

#[tonic::async_trait]
pub trait EvmBehaviour {
    // TODO: better address name
    fn connect(retry_client: OnceCell<RetryClient<RpcServiceClient<InterceptedSvc>>>) -> Self;
    async fn get_receipt(&self, hash: Hash) -> Result<Receipt>;
    async fn get_code(&self, addr: Address) -> Result<ByteCode>;
    async fn get_balance(&self, addr: Address) -> Result<Balance>;
    async fn get_tx_count(&self, addr: Address) -> Result<Nonce>;
    async fn get_abi(&self, addr: Address) -> Result<ByteAbi>;
    async fn estimate_quota(&self, req: CallRequest) -> Result<ByteQuota>;
}

#[tonic::async_trait]
impl EvmBehaviour for EvmClient {
    fn connect(retry_client: OnceCell<RetryClient<RpcServiceClient<InterceptedSvc>>>) -> Self {
        Self { retry_client }
    }

    async fn get_receipt(&self, hash: Hash) -> Result<Receipt> {
        let client = self.retry_client.get().unwrap();
        let hash = CloudHash {
            hash: hash.to_vec(),
        };
        let receipt = client.get_transaction_receipt(hash).await?;
        Ok(receipt)
    }

    async fn get_code(&self, addr: Address) -> Result<ByteCode> {
        let client = self.retry_client.get().unwrap();

        let addr = CloudAddress {
            address: addr.to_vec(),
        };
        client.get_code(addr).await.context("failed to get code")
    }

    async fn get_balance(&self, addr: Address) -> Result<Balance> {
        let client = self.retry_client.get().unwrap();

        let addr = CloudAddress {
            address: addr.to_vec(),
        };
        client
            .get_balance(addr)
            .await
            .context("failed to get balance")
    }

    async fn get_tx_count(&self, addr: Address) -> Result<Nonce> {
        let client = self.retry_client.get().unwrap();

        let addr = CloudAddress {
            address: addr.to_vec(),
        };
        client
            .get_transaction_count(addr)
            .await
            .context("failed to get tx count")
    }

    async fn get_abi(&self, addr: Address) -> Result<ByteAbi> {
        let client = self.retry_client.get().unwrap();

        let addr = CloudAddress {
            address: addr.to_vec(),
        };
        client.get_abi(addr).await.context("failed to get abi")
    }

    async fn estimate_quota(&self, req: CallRequest) -> Result<ByteQuota> {
        let client = self.retry_client.get().unwrap();
        // let req = CallRequest {
        //     from: from.to_vec(),
        //     to: to.to_vec(),
        //     // This is `executor_evm` specific calling convention.
        //     // `executor_chaincode` uses args[0] for payload.
        //     // But since no one uses chaincode, we may just use the evm's convention.
        //     method: data,
        //     args: Vec::new(),
        //     height: 0,
        // };
        client
            .estimate_quota(req)
            .await
            .context("failed to estimate quota")
    }
}

// #[tonic::async_trait]
// pub trait EvmBehaviourExt {
//     async fn store_contract_abi<S>(
//         &self,
//         signer: &S,
//         contract_addr: Address,
//         abi: &[u8],
//         quota: u64,
//         valid_until_block: u64,
//         need_package: bool,
//     ) -> Result<Hash>
//     where
//         S: SignerBehaviour + Send + Sync;
// }
//
// #[tonic::async_trait]
// impl<T> EvmBehaviourExt for T
// where
//     T: TransactionSenderBehaviour + Send + Sync,
// {
//     // The binary protocol is the implementation details of the current EVM service.
//     async fn store_contract_abi<S>(
//         &self,
//         signer: &S,
//         contract_addr: Address,
//         abi: &[u8],
//         quota: u64,
//         valid_until_block: u64,
//         need_package: bool,
//     ) -> Result<Hash>
//     where
//         S: SignerBehaviour + Send + Sync,
//     {
//         let abi_addr = parse_addr(constant::ABI_ADDRESS)?;
//         let data = [contract_addr.as_slice(), abi].concat();
//         let tx_hash = self
//             .send_tx(
//                 signer,
//                 abi_addr.to_vec(),
//                 data,
//                 vec![0; 32],
//                 quota,
//                 valid_until_block,
//                 need_package,
//             )
//             .await?;
//
//         Ok(tx_hash)
//     }
// }

#[cfg(test)]
mod tests {
    use super::constant::*;
    use super::*;
    use crate::common::util::parse_addr;
    use crate::common::util::parse_data;

    #[test]
    fn test_constant() -> Result<()> {
        // TODO: add sm crypto test
        parse_addr(STORE_ADDRESS)?;
        parse_addr(ABI_ADDRESS)?;
        parse_addr(AMEND_ADDRESS)?;

        parse_data(AMEND_ABI)?;
        parse_data(AMEND_CODE)?;
        parse_data(AMEND_KV_H256)?;
        parse_data(AMEND_BALANCE)?;
        Ok(())
    }
}
