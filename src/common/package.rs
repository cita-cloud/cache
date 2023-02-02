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
