use anyhow::Result;
use redis::AsyncCommands;

pub struct CacheService {
    conn: redis::aio::ConnectionManager,
    ttl: u64,
}

impl CacheService {
    pub fn new(conn: redis::aio::ConnectionManager, ttl: u64) -> Self {
        Self { conn, ttl }
    }

    pub async fn get(&mut self, key: &str) -> Result<Option<String>> {
        let value: Option<String> = self.conn.get(key).await?;
        Ok(value)
    }

    pub async fn set(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.set_ex::<_, _, ()>(key, value, self.ttl).await?;
        Ok(())
    }

    pub async fn delete(&mut self, key: &str) -> Result<()> {
        self.conn.del::<_, ()>(key).await?;
        Ok(())
    }

    pub fn generate_cache_key(protocol: &str, chain: &str) -> String {
        format!("rates:{}:{}", protocol, chain)
    }
}
