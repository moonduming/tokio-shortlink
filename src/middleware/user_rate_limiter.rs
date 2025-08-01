use axum::{
    body::Body,
    extract::{State, Extension},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response
};
use tracing::warn;
use std::sync::Arc;
use crate::{
    state::AppState, 
    models::user::User
};
use redis::AsyncCommands;


pub async fn user_rate_limiter(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let key = format!("rate_limit:user:{}", user.id);
    // 从配置中读取限流参数
    let (limit, window_secs) = {
        let config = state.config.read().await;
        (config.user_rate_limit, config.user_rate_limit_window)
    };
    
    // redis 提前释放
    {
        // 获取redis连接
        let mut conn = state.redis_pool.get().await.map_err(|e| {
            warn!("user_rate_limiter: Redis 获取连接失败: err={}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis err: {}", e))
        })?;
        // 限流逻辑
        let count: i64 = conn
            .incr(&key, 1)
            .await
            .map_err(|e| {
                warn!("user_rate_limiter: Redis Incr 失败, user_id={}, err={}", user.id, e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Incr err: {}", e))
            })?;

        if count == 1 {
            // 第一次请求，设置过期时间
            let _: () = conn.expire(&key, window_secs)
                .await
                .map_err(|e| {
                    warn!("user_rate_limiter: Redis Expire 失败, user_id={}, err={}", user.id, e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Expire err: {}", e))
                })?;
        }

        if count > limit {
            warn!("user_rate_limiter: 访问超限, user_id={}, limit={}, window={}", user.id, limit, window_secs);
            // 超出限制
            return Err((StatusCode::TOO_MANY_REQUESTS, "Too many requests".into()));
        }
    }
    
    Ok(next.run(req).await)
}