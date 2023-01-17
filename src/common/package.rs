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
use crate::cita_cloud::evm::constant::STORE_ADDRESS;
use crate::common::crypto::Address;
use crate::common::util::{parse_addr, parse_value};
use crate::rest_api::post::PackageTx;
use anyhow::Result;
use cita_cloud_proto::blockchain::Block;
use msgpack_schema::{serialize, Deserialize, Serialize};
use prost::Message;

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
pub struct Package {
    #[tag = 0]
    pub batch_number: u64,
    #[tag = 1]
    pub block: Vec<u8>,
}

impl Package {
    pub fn new(batch_number: u64, block: Block) -> Package {
        let mut block_bytes = Vec::new();
        block.encode(&mut block_bytes).expect("encode block failed");
        Self {
            batch_number,
            block: block_bytes,
        }
    }

    pub fn to_packaged_tx(&self, from: Address) -> Result<PackageTx> {
        Ok(PackageTx {
            from,
            to: parse_addr(STORE_ADDRESS)?,
            data: serialize(self.clone()),
            value: parse_value("0x0")?.to_vec(),
            block_count: 20,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::cita_cloud::controller::SignerBehaviour;
    use crate::cita_cloud::wallet::{MaybeLocked, MultiCryptoAccount};
    use crate::common::package::Package;
    use crate::common::util::timestamp;
    use cita_cloud_proto::blockchain::{
        Block, BlockHeader, RawTransaction, RawTransactions, Transaction as CloudNormalTransaction,
    };
    use msgpack_schema::{deserialize, serialize};
    use prost::Message;
    use tokio_test;

    fn sign(tx: CloudNormalTransaction) -> RawTransaction {
        tokio_test::block_on(async {
            let account_str = "crypto_type = 'SM'\naddress = '0x36fbd539ca0ade15bcfc453a79f5f33450749b51'\npublic_key = '0xc4162ace569f6e82be31b47b0ba4aa0cd991ef10dc98397aedc16fecf86636e4e6154649fe07671edc7c1965e87eefa1bd65018fcb39f16729cd246250b1116e'\nsecret_key = '0x67aef6c49c853f6022e27d2099aeeaf732f41a76770855b3837b8c000b4b945e'\n";
            let maybe: MaybeLocked = toml::from_str(&account_str).unwrap();
            let account: &MultiCryptoAccount = maybe.unlocked().unwrap();
            account.sign_raw_tx(tx).await
        })
    }

    fn get_block() -> Block {
        let header = BlockHeader {
            prevhash: Vec::new(),
            timestamp: timestamp(),
            height: 0,
            transactions_root: Vec::new(),
            proposer: Vec::new(),
        };
        let tx = CloudNormalTransaction {
            version: 0u32,
            to: Vec::new(),
            data: Vec::new(),
            value: Vec::new(),
            nonce: rand::random::<u64>().to_string(),
            quota: 20000,
            valid_until_block: 20,
            chain_id: "1".into(),
        };

        let mut tx_list = Vec::new();
        tx_list.push(sign(tx));
        Block {
            version: 0,
            header: Some(header.clone()),
            body: Some(RawTransactions { body: tx_list }),
            proof: vec![],
            state_root: vec![],
        }
    }

    #[test]
    fn test_serialize() {
        let full_block = get_block();
        let mut block_bytes = Vec::new();
        full_block
            .encode(&mut block_bytes)
            .expect("encode block failed");
        let content = Package {
            batch_number: 1,
            block: block_bytes,
        };
        let encoded = serialize(content);
        let decoded_package = deserialize::<Package>(encoded.as_slice()).unwrap();
        let decoded_block = Block::decode(decoded_package.block.as_slice()).unwrap();
        assert_eq!(full_block, decoded_block);
    }

    #[test]
    fn test_replay() {}
}
