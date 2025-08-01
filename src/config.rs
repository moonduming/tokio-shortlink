use serde::Deserialize;
use config::{Config, Environment, ConfigError};
use dotenvy;
use std::env;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// MySQL 连接字符串
    pub database_url: String,
    /// Redis 连接字符串
    pub redis_url: String,
    /// 服务地址
    pub addr: String,
    /// JWT 密钥
    pub jwt_secret: String,
    /// 用户 token 的过期时间
    pub user_token_ttl: i64,
    /// 短链的最小过期时间
    pub shortlink_min_ttl: i64,
    /// 短链的最大过期时间
    pub shortlink_max_ttl: i64,
    /// Redis 的最大过期时间
    pub redis_max_ttl: i64,
    /// Redis 的最小缓存时间
    pub redis_min_cache_ttl: i64,
    /// 最大统计天数
    pub max_stats_days: u8,
    /// IP 限流
    pub ip_rate_limit: i64,
    /// IP 限流时间窗口（秒）
    pub ip_rate_limit_window: i64,
    /// 账号连续失败次数阈值
    pub user_login_fail_limit: i64,
    /// 账号失败锁定时长（秒）
    pub user_login_fail_ttl: i64,
    /// 单 IP + 账号连续失败次数阈值
    pub ip_user_login_fail_limit: i64,
    /// 单 IP + 账号失败锁定时长（秒）
    pub ip_user_login_fail_ttl: i64,
    /// 注册接口 - 每个IP每日注册次数上限
    pub ip_register_limit: i64,
    /// 注册接口 - 注册计数窗口（秒），86400=1天
    pub ip_register_ttl: i64,
    /// 用户限流
    pub user_rate_limit: i64,
    /// 用户 token 限流
    pub user_token_limit: u8,
    /// 用户限流时间窗口（秒）
    pub user_rate_limit_window: i64,
    /// 全局 HTTP 超时时间（毫秒）
    pub global_timeout_ms: u64,
    /// 最大 MySQL 连接数
    pub mysql_max_connections: u32,
    /// 等待连接池中空闲连接的超时时间（毫秒）
    pub mysql_acquire_timeout_ms: u64,
    /// 单个查询语句的最大执行时间（毫秒）
    pub mysql_query_timeout_ms: u64,
    /// InnoDB 表中等待锁的最大时间（秒）
    pub mysql_lock_wait_timeout_s: u64,

    /// Redis 连接池最大连接数
    pub redis_pool_size: usize,
    /// 等待空闲连接的最大时间（毫秒）
    pub redis_timeout_wait_ms: u64,
    /// 新建连接的最大时间（毫秒）
    pub redis_timeout_create_ms: u64,
    /// 取连接前健康检查的超时时间（毫秒）
    pub redis_timeout_recycle_ms: u64,
    /// Redis 背台作业队列容量
    pub bg_redis_queue_cap: usize,
    /// Redis 背台作业最大并发数
    pub bg_redis_max_concurrency: usize,
    /// 过期短链删除任务的执行间隔（秒）
    pub bg_expired_links_sync_interval: u64,
    /// 点击量同步任务的执行间隔（秒）
    pub bg_click_counts_sync_interval: u64,
    /// 访问日志同步任务的执行间隔（秒）
    pub bg_visit_logs_sync_interval: u64,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        // 根据 ENV_FILE 环境变量指定的文件加载环境变量，默认使用 ".env"
        let env_file = env::var("ENV_FILE").unwrap_or_else(|_| ".env".to_string());
        dotenvy::from_filename(&env_file).ok();
        Config::builder()
            .add_source(Environment::default())
            .build()?
            .try_deserialize()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_app_config() {
        // 设置必要的环境变量，模拟 .env
        unsafe {
            env::set_var("DATABASE_URL", "mysql://root:66787@localhost:3306/shortlink");
            env::set_var("REDIS_URL", "redis://127.0.0.1/");
            env::set_var("ADDR", "127.0.0.1:3000");
            env::set_var("JWT_SECRET", "secret");
            env::set_var("USER_TOKEN_TTL", "3600");
            env::set_var("SHORTLINK_MIN_TTL", "60");
            env::set_var("SHORTLINK_MAX_TTL", "3600");
            env::set_var("REDIS_MAX_TTL", "86400");
            env::set_var("REDIS_MIN_CACHE_TTL", "60");
            env::set_var("MAX_STATS_DAYS", "30");
            env::set_var("IP_RATE_LIMIT", "100");
            env::set_var("IP_RATE_LIMIT_WINDOW", "60");
            env::set_var("USER_LOGIN_FAIL_LIMIT", "5");
            env::set_var("USER_LOGIN_FAIL_TTL", "900");
            env::set_var("IP_USER_LOGIN_FAIL_LIMIT", "3");
            env::set_var("IP_USER_LOGIN_FAIL_TTL", "120");
            env::set_var("IP_REGISTER_LIMIT", "5");
            env::set_var("IP_REGISTER_TTL", "86400");
            env::set_var("USER_RATE_LIMIT", "200");
            env::set_var("USER_RATE_LIMIT_WINDOW", "60");
            env::set_var("GLOBAL_TIMEOUT_MS", "2000");
            env::set_var("MYSQL_MAX_CONNECTIONS", "5");
            env::set_var("MYSQL_ACQUIRE_TIMEOUT_MS", "600");
            env::set_var("MYSQL_QUERY_TIMEOUT_MS", "800");
            env::set_var("MYSQL_LOCK_WAIT_TIMEOUT_S", "5");
            env::set_var("REDIS_POOL_SIZE", "4");
            env::set_var("REDIS_TIMEOUT_WAIT_MS", "300");
            env::set_var("REDIS_TIMEOUT_CREATE_MS", "500");
            env::set_var("REDIS_TIMEOUT_RECYCLE_MS", "200");
        }

        let cfg = AppConfig::from_env().expect("load config");

        assert_eq!(
            cfg.database_url,
            "mysql://root:66787@localhost:3306/shortlink"
        );
        assert_eq!(cfg.redis_url, "redis://127.0.0.1/");
        assert_eq!(cfg.ip_rate_limit, 100);
        assert_eq!(cfg.user_rate_limit_window, 60);
        assert_eq!(cfg.ip_register_limit, 5);
        assert_eq!(cfg.user_rate_limit, 200);
        assert_eq!(cfg.global_timeout_ms, 2000);
    }
}
