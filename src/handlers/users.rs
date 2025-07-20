use axum::{extract::{ConnectInfo, State}, http::StatusCode, Json};
use serde::Deserialize;
use validator::Validate;
use std::{sync::Arc, net::SocketAddr};
use crate::{
    state::AppState, 
    services::{UserService, LoginResp}
};
use tracing::warn;


#[derive(Deserialize, Debug, Validate)]
pub struct UserPayload {
    #[validate(length(min = 2, max = 30))]
    pub nickname: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters long"))]
    pub password: String,

    #[validate(email)]
    pub email: String,
}


#[derive(Deserialize, Debug, Validate)]
pub struct LoginPayload {
    #[validate(email)]
    pub email: String,
    pub password: String,
}


/// 注册
pub async fn register(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>, 
    Json(payload): Json<UserPayload>,
) -> Result<(), (StatusCode, String)> {
    let ip = addr.ip().to_string();
    payload.validate().map_err(|e| {
        warn!("register: 用户注册参数校验失败: ip={}, email={}, error={}", ip, payload.email, e);
        (StatusCode::BAD_REQUEST, format!("Validation error: {}", e))
    })?;

    UserService::register(
        &state, 
        &payload.nickname, 
        &payload.password, 
        &payload.email,
        &ip
    ).await?;

    Ok(())
}


/// 登录
pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>, 
    Json(payload): Json<LoginPayload>,
) -> Result<Json<LoginResp>, (StatusCode, String)> {
    let ip: String = addr.ip().to_string();
    payload.validate().map_err(|e| {
        warn!("login: 用户登录参数校验失败: ip={}, email={}, error={}", ip, payload.email, e);
        (StatusCode::BAD_REQUEST, format!("Validation error: {}", e))
    })?;

    let resp = UserService::login(
        &state, 
        &payload.email, 
        &payload.password,
        &ip
    ).await?;

    Ok(Json(resp))
}
