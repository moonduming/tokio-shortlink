use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use redis::Client as RedisClient;

/// 创建 MySQL 连接池
pub async fn new_mysql_pool(database_url: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

/// 创建 Redis 连接
pub async fn new_redis_client(redis_url: &str) -> Result<RedisClient, redis::RedisError> {
    RedisClient::open(redis_url)
}
