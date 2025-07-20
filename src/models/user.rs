use serde::{Serialize, Deserialize};
use sqlx::MySqlPool;
use axum::http::StatusCode;
use redis::{aio::ConnectionManager, AsyncCommands};
use tracing::warn;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: u64,
    pub email: String,
    pub nickname: Option<String>,
    pub password: String,
    pub status: i8,
}


impl User {
    /// 判断邮箱是否已经注册
    pub async fn exists_by_email(
        mysql_pool: &MySqlPool,
        email: &str
    ) -> Result<bool, (StatusCode, String)> {
        let exists = sqlx::query_scalar!(
            "SELECT 1 FROM users WHERE email = ? LIMIT 1",
            email,
        )
        .fetch_optional(mysql_pool)
        .await
        .map_err(|e| {
            warn!("exists_by_email: DB select error: email={}, err={}", email, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB select error: {}", e))
        })?
        .is_some();

        Ok(exists)
    }

    /// 创建用户
    pub async fn create(
        mysql_pool: &MySqlPool,
        nickname: &str,
        password: &str,
        email: &str,
    ) -> Result<(), (StatusCode, String)> {
        sqlx::query!(
            "INSERT INTO users (nickname, password, email) VALUES (?, ?, ?)",
            nickname,
            password,
            email,
        )
        .execute(mysql_pool)
        .await
        .map_err(|e| {
            warn!("create: DB insert error: email={}, err={}", email, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB insert error: {}", e))
        })?;

        Ok(())
    }

    /// 根据 id 或 email 查询用户
    pub async fn find_user(
        mysql_pool: &MySqlPool,
        id: Option<u64>,
        email: Option<&str>,
    ) -> Result<Option<User>, (StatusCode, String)> {

        // run the correct query and await inside each branch so the match arms have the same type
        let row = match (id, email) {
            (Some(id), None) => {
                sqlx::query_as!(
                    User,
                    "SELECT id, email, nickname, password, status FROM users WHERE id = ? LIMIT 1",
                    id
                )
                .fetch_optional(mysql_pool)
                .await
            }
            (None, Some(email)) => {
                sqlx::query_as!(
                    User,
                    "SELECT id, email, nickname, password, status FROM users WHERE email = ? LIMIT 1",
                    email
                )
                .fetch_optional(mysql_pool)
                .await
            }
            _ => {
                warn!("find_user: 参数错误: id={:?}, email={:?}", id, email);
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid parameters: require either id or email".into(),
                ));
            }
        }
        .map_err(|e| {
            warn!("find_user: DB select error: id={:?}, email={:?}, err={}", id, email, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB select error: {e}"),
            )
        })?;

        Ok(row)
    }

    /// 读取次数
    async fn read_count(
        redis_mgr: &mut ConnectionManager,
        key: &str,
    ) -> Result<i64, (StatusCode, String)> {
        let cnt = redis_mgr.get::<_,Option<i64>>(key).await.map_err(|e| {
            warn!("read_count: Redis get error: key={}, err={}", key, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis get error: {}", e))
        })?.unwrap_or(0);

        Ok(cnt)
    }

    /// 检查次数是否超过限制
    async fn check_limit(
        redis_mgr: &mut ConnectionManager,
        key: &str,
        limit: i64,
    ) -> Result<bool, (StatusCode, String)> {
        let cnt = Self::read_count(redis_mgr, key).await?;
        Ok(cnt >= limit)
    }

    async fn incr_count(
        redis_mgr: &mut ConnectionManager,
        key: &str,
        ttl: i64,
    ) -> Result<(), (StatusCode, String)> {
        let count: i64 = redis_mgr
        .incr(&key, 1)
        .await
        .map_err(|e| {
            warn!("incr_count: Redis Incr err: key={}, err={}", key, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Incr err: {}", e))
        })?;

        if count == 1 {
            // 设置登录失败计数过期时间
            let _: () = redis_mgr.expire(&key, ttl)
            .await
            .map_err(|e| {
                warn!("incr_count: Redis Expire err: key={}, err={}", key, e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Expire err: {}", e))
            })?;
        }

        Ok(())
    }

    /// 判断用户是否可以登录
    pub async fn can_login(
        redis_mgr: &mut ConnectionManager,
        user_login_fail_limit: i64,
        ip_user_login_fail_limit: i64,
        user_fail_key: &str,
        ip_user_fail_key: &str,
    ) -> Result<(), (StatusCode, String)> {
        // 只读取计数，不再自增；真正失败后再单独调用记录函数
        if Self::check_limit(
            redis_mgr, 
            user_fail_key, 
            user_login_fail_limit,
        ).await? {
            warn!("can_login: 用户登录被限流: user_fail_key={}", user_fail_key);
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Account temporarily locked due to multiple failed login attempts".into(),
            ));
        }

        if Self::check_limit(
            redis_mgr, 
            ip_user_fail_key, 
            ip_user_login_fail_limit,
        ).await? {
            warn!("can_login: IP 登录被限流: ip_user_fail_key={}", ip_user_fail_key);
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Too many login attempts from this device, please try again later".into(),
            ));
        }

        Ok(())
    }

    /// 记录登录失败
    pub async fn record_login_fail(
        redis_mgr: &mut ConnectionManager,
        user_fail_key: &str,
        ip_user_fail_key: &str,
        user_login_fail_ttl: i64,
        ip_user_login_fail_ttl: i64,
    ) -> Result<(), (StatusCode, String)> {
        Self::incr_count(
            redis_mgr, 
            user_fail_key, 
            user_login_fail_ttl,
        ).await?;

        Self::incr_count(
            redis_mgr, 
            ip_user_fail_key, 
            ip_user_login_fail_ttl,
        ).await?;

        Ok(())
    }

    /// 登录成功
    pub async fn login_success(
        redis_mgr: &mut ConnectionManager,
        user_fail_key: &str,
        ip_user_fail_key: &str,
    ) -> Result<(), (StatusCode, String)> {
        let _: () = redis_mgr.del(user_fail_key)
        .await
        .map_err(|e| {
            warn!("login_success: Redis Del err: key={}, err={}", user_fail_key, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Del err: {}", e))
        })?;

        let _: () = redis_mgr.del(ip_user_fail_key)
        .await
        .map_err(|e| {
            warn!("login_success: Redis Del err: key={}, err={}", ip_user_fail_key, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Del err: {}", e))
        })?;

        Ok(())
    }

    /// 检查当前 IP 是否超过注册次数限制
    pub async fn can_register(
        redis_mgr: &mut ConnectionManager,
        ip_register_limit: i64,
        ip_register_key: &str,
    ) -> Result<(), (StatusCode, String)> {
        if Self::check_limit(redis_mgr, ip_register_key, ip_register_limit).await? {
            warn!("can_register: 注册被限流: ip_register_key={}", ip_register_key);
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Too many registration attempts from this device, please try again later".into(),
            ));
        }
        Ok(())
    }

    /// 记录注册次数
    pub async fn record_register(
        redis_mgr: &mut ConnectionManager,
        ip_register_key: &str,
        ip_register_ttl: i64,
    ) -> Result<(), (StatusCode, String)> {
        Self::incr_count(redis_mgr, ip_register_key, ip_register_ttl).await
    }
}