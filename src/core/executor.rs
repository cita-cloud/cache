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

use anyhow::{Context, Result};
use cita_cloud_proto::client::{ExecutorClientTrait, InterceptedSvc};

use crate::crypto::{Address, ArrayLike};
use cita_cloud_proto::executor::{
    executor_service_client::ExecutorServiceClient, CallRequest, CallResponse,
};
use cita_cloud_proto::retry::RetryClient;
use tokio::sync::OnceCell;

#[derive(Debug, Clone)]
pub struct ExecutorClient {
    retry_client: OnceCell<RetryClient<ExecutorServiceClient<InterceptedSvc>>>,
}

#[tonic::async_trait]
pub trait ExecutorBehaviour {
    fn connect(retry_client: OnceCell<RetryClient<ExecutorServiceClient<InterceptedSvc>>>) -> Self;
    async fn call(
        &self,
        from: Address,
        to: Address,
        data: Vec<u8>,
        height: u64,
    ) -> Result<CallResponse>;
}

#[tonic::async_trait]
impl ExecutorBehaviour for ExecutorClient {
    fn connect(retry_client: OnceCell<RetryClient<ExecutorServiceClient<InterceptedSvc>>>) -> Self {
        Self { retry_client }
    }

    async fn call(
        &self,
        from: Address,
        to: Address,
        data: Vec<u8>,
        height: u64,
    ) -> Result<CallResponse> {
        let client = self.retry_client.get().unwrap();
        let req = CallRequest {
            from: from.to_vec(),
            to: to.to_vec(),
            // This is `executor_evm` specific calling convention.
            // `executor_chaincode` uses args[0] for payload.
            // But since no one uses chaincode, we may just use the evm's convention.
            method: data,
            args: Vec::new(),
            height,
        };
        client
            .call(req)
            .await
            .context("failed to do executor gRPC call")
    }
}
