use axum::{
    body::Body, 
    extract::State, 
    http::{Request, StatusCode}, 
    middleware::Next, 
    response::Response
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::sync::Arc;
use crate::{
    state::AppState, 
    models::user::User, 
    services::Claims
};
use redis::AsyncCommands;
use tracing::warn;


pub async fn jwt_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // 提取 Bearer token
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| {
            warn!("jwt_auth: 缺少 Authorization header");
            (
                StatusCode::UNAUTHORIZED, 
                "Missing token".into()
            )
        })?;

    // 校验 JWT 是否过期
    let jwt_secret = {
        let cfg = state.config.read().await;
        cfg.jwt_secret.clone()
    };
    
    let claims = decode::<Claims>(
        token, 
        &DecodingKey::from_secret(jwt_secret.as_bytes()), 
        &Validation::new(Algorithm::HS256)
    )
    .map_err(|e| {
        warn!("jwt_auth: JWT 校验失败: {}", e);
        (StatusCode::UNAUTHORIZED, format!("JWT err: {}", e))
    })?;

    let key = format!("session:{}", claims.claims.jti);

    // 构建作用域，让 conn 在作用域结束时自动释放
    {
        let mut conn = state.redis_pool.get().await.map_err(|e| {
            warn!("jwt_auth: Redis 获取连接失败: err={}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis err: {}", e))
        })?;

        let exists: bool = conn.exists(&key).await.map_err(|e| {
            warn!("jwt_auth: Redis 查询失败: key={}, err={}", key, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis err: {}", e))
        })?;

        if !exists {
            warn!("jwt_auth: Redis session 不存在或已过期: key={}", key);
            return Err((StatusCode::UNAUTHORIZED, "Token expired".into()));
        }
    }
    
    let user = match User::find_user(
        &state.mysql_pool, 
        Some(claims.claims.sub), 
        None).await? {
            Some(user) => user,
            None => {
                warn!("jwt_auth: 用户不存在: user_id={}", claims.claims.sub);
                return Err((StatusCode::NOT_FOUND, "User not found".into()));
            },
        };

    
    if user.status != 1 {
        warn!("jwt_auth: 用户已被禁用: user_id={}", user.id);
        return Err((StatusCode::UNAUTHORIZED, "User is disabled".into()));
    }

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}