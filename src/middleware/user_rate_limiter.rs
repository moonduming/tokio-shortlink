use axum::{
    body::Body, 
    extract::{State, Extension}, 
    http::{Request, StatusCode}, 
    middleware::Next, 
    response::Response
};
use std::sync::Arc;
use crate::{AppState, models::user::User};
use rand::{rng, seq::IndexedRandom};
use redis::AsyncCommands;


pub async fn user_rate_limiter(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    
    let key = format!("rate_limit:user:{}", user.id);
    // 从配置中读取限流参数
    let config = state.config.read().await;
    let limit = config.user_rate_limit;
    let window_secs = config.user_rate_limit_window;
    
    // 获取redis连接
    let manager = state.managers
        .choose(&mut rng())
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into()))?;
    let mut conn = manager.lock().await;

    // 限流逻辑
    let count: i64 = conn
        .incr(&key, 1)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Incr err: {}", e))
        })?;

    if count == 1 {
        // 第一次请求，设置过期时间
        let _: () = conn.expire(&key, window_secs)
            .await
            .map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Expire err: {}", e))
            })?;
    }
    
    if count > limit {
        // 超出限制
        return Err((StatusCode::TOO_MANY_REQUESTS, "Too many requests".into()));
    }
    
    Ok(next.run(req).await)
}