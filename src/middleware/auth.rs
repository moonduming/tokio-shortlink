use axum::{
    body::Body, 
    extract::State, 
    http::{Request, StatusCode}, 
    middleware::Next, 
    response::Response
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::sync::Arc;
use crate::{AppState, models::user::User, services::Claims};
use rand::{rng, seq::IndexedRandom};
use redis::AsyncCommands;


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
        .ok_or(
            (
                StatusCode::UNAUTHORIZED, 
                "Missing token".into()
            )
        )?;

    // 校验 JWT 是否过期
    let config = state.config.read().await;
    let claims = decode::<Claims>(
        token, 
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()), 
        &Validation::new(Algorithm::HS256)
    )
    .map_err(|e| {
        (StatusCode::UNAUTHORIZED, format!("JWT err: {}", e))
    })?;

    let key = format!("session:{}", claims.claims.jti);
    // 随机选择一个 Redis 连接
    let manager = state.managers
        .choose(&mut rng())
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into()))?;
    let mut conn = manager.lock().await;

    let exists: bool = conn.exists(&key).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis err: {}", e))
    })?;

    if !exists {
        return Err((StatusCode::UNAUTHORIZED, "Token expired".into()));
    }
    
    let user = match User::find_user(
        &state.mysql_pool, 
        Some(claims.claims.sub), 
        None).await? {
            Some(user) => user,
            None => return Err((StatusCode::NOT_FOUND, "User not found".into())),
        };

    
    if user.status != 1 {
        return Err((StatusCode::UNAUTHORIZED, "User is disabled".into()));
    }

    req.extensions_mut().insert(user);

    Ok(next.run(req).await)
}