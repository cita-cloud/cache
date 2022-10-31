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

use cita_cloud_proto::client::InterceptedSvc;

use cita_cloud_proto::crypto::crypto_service_client::CryptoServiceClient;
use cloud_util::crypto::{hash_data, sign_message};

use tokio::sync::OnceCell;

#[derive(Debug, Clone)]
pub struct CryptoClient {
    retry_client:
        OnceCell<cita_cloud_proto::retry::RetryClient<CryptoServiceClient<InterceptedSvc>>>,
}

#[tonic::async_trait]
pub trait CryptoBehaviour {
    fn connect(
        retry_client: OnceCell<
            cita_cloud_proto::retry::RetryClient<CryptoServiceClient<InterceptedSvc>>,
        >,
    ) -> Self;
    async fn hash_data(&self, data: Vec<u8>) -> Vec<u8>;

    async fn sign_message(&self, data: Vec<u8>) -> Vec<u8>;
}

#[tonic::async_trait]
impl CryptoBehaviour for CryptoClient {
    fn connect(
        retry_client: OnceCell<
            cita_cloud_proto::retry::RetryClient<CryptoServiceClient<InterceptedSvc>>,
        >,
    ) -> Self {
        Self { retry_client }
    }

    async fn hash_data(&self, data: Vec<u8>) -> Vec<u8> {
        hash_data(self.retry_client.get().cloned().unwrap(), &data)
            .await
            .unwrap()
    }

    async fn sign_message(&self, msg: Vec<u8>) -> Vec<u8> {
        sign_message(self.retry_client.get().cloned().unwrap(), &msg)
            .await
            .unwrap()
    }
}
