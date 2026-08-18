use redis::aio::ConnectionManager;
use redis::AsyncCommands;

#[derive(Clone)]
pub struct RedisCache {
    client: ConnectionManager,
}

impl RedisCache {
    pub async fn new(url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(url)?;
        let manager = ConnectionManager::new(client).await?;
        Ok(Self { client: manager })
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, redis::RedisError> {
        let mut conn = self.client.clone();
        conn.get(key).await
    }

    pub async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_secs: u64,
    ) -> Result<(), redis::RedisError> {
        let mut conn = self.client.clone();
        conn.set_ex(key, value, ttl_secs).await
    }

    pub async fn delete(&self, key: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.client.clone();
        conn.del(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_initialization_error() {
        // Invalid schema to trigger RedisError immediately without hanging
        let res = RedisCache::new("invalid://localhost:9999").await;
        assert!(res.is_err());
    }
}
