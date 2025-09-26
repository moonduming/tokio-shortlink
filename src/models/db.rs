use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use sqlx::Executor;
use std::time::Duration;
use deadpool_redis::{Config, Pool, PoolConfig};

/// 创建 MySQL 连接池
pub async fn new_mysql_pool(
    url: &str,
    max_connections: u32,
    timeout:u64,
    max_execution_time: u64,
    innodb_lock_wait_timeout: u64,
) -> Result<MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_millis(timeout))
        .after_connect(move |conn, _meta| {
            // capture by value so the closure is 'static and can outlive the function
            let max_execution_time = max_execution_time;
            let innodb_lock_wait_timeout = innodb_lock_wait_timeout;
            Box::pin(async move {
                conn.execute(sqlx::query(
                    &format!("SET SESSION max_execution_time = {}", max_execution_time)
                )).await?;
                conn.execute(sqlx::query(
                    &format!("SET SESSION innodb_lock_wait_timeout = {}", innodb_lock_wait_timeout)
                )).await?;
                Ok(())
            })
        })
        .connect(url)
        .await
}


/// 创建 Redis 连接池
pub fn new_redis_pool(
    redis_url: &str,
    max_connections: usize,
    t_wait: u64,
    t_create: u64,
    t_recycle: u64,
) -> Result<Pool, deadpool_redis::CreatePoolError> {
    let mut cfg = Config::from_url(redis_url);
    // 配置连接池超时：等待空闲连接 300ms，新建连接超时 500ms
    let mut pool_cfg = PoolConfig::new(max_connections);
    pool_cfg.timeouts.wait = Some(Duration::from_millis(t_wait));  // 等待空闲连接 300ms
    pool_cfg.timeouts.create = Some(Duration::from_millis(t_create));  // 新建连接超时 500ms
    pool_cfg.timeouts.recycle = Some(Duration::from_millis(t_recycle));  // 取连接前健康检查超时 200ms
    cfg.pool = Some(pool_cfg);
    cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))
}
