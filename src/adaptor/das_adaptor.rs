use crate::redis::set;
use crate::{config, delete, get, Pool};
use anyhow::{anyhow, Result};
use tikv_client::RawClient;

#[tonic::async_trait]
pub trait DasAdaptor {
    async fn new() -> Self
    where
        Self: Sized;

    async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()>;

    async fn get(&self, key: Vec<u8>) -> Result<Vec<u8>>;

    async fn delete(&self, key: Vec<u8>) -> Result<()>;
}

#[derive(Clone)]
pub struct Redis {
    pool: Pool,
}

#[tonic::async_trait]
impl DasAdaptor for Redis {
    async fn new() -> Self
    where
        Self: Sized,
    {
        let config = config();
        let pool = Pool::new_with_uri(format!("{}/1", config.redis_addr.unwrap()));
        Self { pool }
    }

    async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        set(&mut self.pool.get(), key, value).expect("redis save error");
        Ok(())
    }

    async fn get(&self, key: Vec<u8>) -> Result<Vec<u8>> {
        Ok(get(&mut self.pool.get(), key)?)
    }

    async fn delete(&self, key: Vec<u8>) -> Result<()> {
        delete(&mut self.pool.get(), key).expect("redis delete error");
        Ok(())
    }
}

#[derive(Clone)]
pub struct Tikv {
    client: RawClient,
}

#[tonic::async_trait]
impl DasAdaptor for Tikv {
    async fn new() -> Self
    where
        Self: Sized,
    {
        let config = config();
        let client = RawClient::new(config.tikv_addr.unwrap())
            .await
            .expect("get tikv client error");
        Self { client }
    }

    async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        Ok(self.client.put(key, value).await?)
    }

    async fn get(&self, key: Vec<u8>) -> Result<Vec<u8>> {
        match self.client.get(key).await {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn delete(&self, key: Vec<u8>) -> Result<()> {
        Ok(self.client.delete(key).await?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u64)]
pub enum DasType {
    Redis = 0,
    Tikv,
}

impl TryFrom<u64> for DasType {
    type Error = ();

    fn try_from(v: u64) -> Result<Self, Self::Error> {
        match v {
            x if x == DasType::Redis as u64 => Ok(DasType::Redis),
            x if x == DasType::Tikv as u64 => Ok(DasType::Tikv),
            _ => Err(()),
        }
    }
}
#[derive(Clone)]
pub struct Das {
    tikv: Option<Tikv>,
    redis: Option<Redis>,
}

#[tonic::async_trait]
impl DasAdaptor for Das {
    async fn new() -> Self
    where
        Self: Sized,
    {
        let config = config();
        match DasType::try_from(config.das_type).expect("das type invalid!") {
            DasType::Redis => Self {
                redis: Some(Redis::new().await),
                tikv: None,
            },
            DasType::Tikv => Self {
                redis: None,
                tikv: Some(Tikv::new().await),
            },
        }
    }

    async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        let config = config();

        match DasType::try_from(config.das_type).expect("das type invalid!") {
            DasType::Redis => self.redis.as_ref().unwrap().put(key, value).await,
            DasType::Tikv => self.tikv.as_ref().unwrap().put(key, value).await,
        }
    }

    async fn get(&self, key: Vec<u8>) -> Result<Vec<u8>> {
        let config = config();
        match DasType::try_from(config.das_type).expect("das type invalid!") {
            DasType::Redis => self.redis.as_ref().unwrap().get(key).await,
            DasType::Tikv => self.tikv.as_ref().unwrap().get(key).await,
        }
    }

    async fn delete(&self, key: Vec<u8>) -> Result<()> {
        let config = config();
        match DasType::try_from(config.das_type).expect("das type invalid!") {
            DasType::Redis => self.redis.as_ref().unwrap().delete(key).await,
            DasType::Tikv => self.tikv.as_ref().unwrap().delete(key).await,
        }
    }
}
