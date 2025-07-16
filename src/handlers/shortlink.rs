use axum::{
    extract::{ConnectInfo, Path, Query, State}, 
    http::StatusCode, 
    response::Redirect, 
    Extension, 
    Json
};
use axum_extra::TypedHeader;
use chrono::NaiveDateTime;
use headers::{UserAgent, Referer};
use std::{sync::Arc, net::SocketAddr};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{AppState, services::ShortlinkService, models::{user::User, link::LinkDto}};


/// 客户端请求：创建短链
#[derive(Deserialize, Validate)]
pub struct ShortlinkCreateReq {
    #[validate(url(message = "Invalid URL"))]
    pub url: String,
    pub ttl: Option<i64>,
    pub short_code: Option<String>,
}

/// 服务端返回：短链创建结果
#[derive(Serialize)]
pub struct ShortlinkCreateResp {
    pub short_url: String,
}



/// 查询参数
#[derive(Debug, Default, Deserialize, Validate)]
pub struct LinkQuery {
    // ---筛选条件---
    pub user_id: Option<u64>, // 用户ID
    pub short_code: Option<String>, // 短码
    pub long_url: Option<String>, // 长 URL
    pub click_count: Option<u64>, // 点击量
    pub date_from:    Option<NaiveDateTime>, // 日期范围
    pub date_to:      Option<NaiveDateTime>,
    // ---分页---
    #[validate(range(min = 1, max = 100, message = "Limit must be between 1 and 100"))]
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}

fn default_limit() -> u64 { 10 }


/// 返回数据
#[derive(Serialize)]
pub struct LinkList {
    pub links: Vec<LinkDto>,
    pub count: i64,
}


/// 删除短链请求
#[derive(Deserialize, Validate)]
pub struct DeleteLinksReq {
    #[validate(length(min = 1, max = 50, message = "Ids must be between 1 and 50"))]
    pub ids: Vec<u64>,
}


/// 点击量统计（按天）
#[derive(Debug, Deserialize, Validate)]
pub struct LinkStatsQuery {
    pub short_code: String,   // 必填：要统计哪条短链
    #[serde(default = "default_days")]
    #[validate(range(min = 1, message = "Days must be greater than 0"))]
    pub days: u8,             // 选填：统计最近多少天
}

fn default_days() -> u8 { 30 }


/// 创建短链
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<ShortlinkCreateReq>,
) -> Result<Json<ShortlinkCreateResp>, (StatusCode, String)> {
    // 校验 url
    if let Err(e) = payload.validate() {
        return Err((StatusCode::BAD_REQUEST, format!("Validation error: {}", e)));
    }

    // 校验短链有效时间
    let config = state.config.read().await;
    let min_ttl = config.shortlink_min_ttl;
    let max_ttl = config.shortlink_max_ttl;

    let ttl = match payload.ttl {
        Some(ttl) => {
            if ttl < min_ttl || ttl > max_ttl {
                return Err((
                    StatusCode::BAD_REQUEST, 
                    format!("TTL must be between {} and {}", min_ttl, max_ttl)
                ));
            }
            ttl
        },
        None => min_ttl,
    };

    // 创建短链
    let short_url = ShortlinkService::create_shortlink(
        &state, 
        &payload.url,
        payload.short_code,
        ttl,
        user.id
    ).await?;
    
    Ok(Json(ShortlinkCreateResp { short_url }))
}

/// 重定向
pub async fn redirect(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    referer: Option<TypedHeader<Referer>>,
    Path(short_code): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Redirect, (StatusCode, String)> {
    let ip = addr.ip().to_string();
    let ua = user_agent.as_str();
    let ref_ = referer.map(|r| r.to_string()).unwrap_or_default();
    let long_url = ShortlinkService::get_long_url(
        &ip, 
        ua, 
        &ref_, 
        &state, 
        &short_code
    ).await?;
    
    Ok(Redirect::to(&long_url))
}

/// 获取短链列表
pub async fn list_links(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(mut q): Query<LinkQuery>,
) -> Result<Json<LinkList>, (StatusCode, String)> {
    q.user_id = Some(user.id);
    let (links, count) = ShortlinkService::list_links(
        &state,
        &q,
        q.limit,
        q.offset,
    ).await?;

    Ok(Json(LinkList { links, count }))
}

/// 删除短链
pub async fn delete_links(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<DeleteLinksReq>,
) -> Result<(), (StatusCode, String)> {
    ShortlinkService::delete_links(
        &state,
        payload.ids,
        user.id,
    ).await?;

    Ok(())
}

/// 点击量统计（按天）
pub async fn get_link_stats(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(q): Query<LinkStatsQuery>,
) -> Result<Json<Vec<(String, i64)>>, (StatusCode, String)> {
    let stats = ShortlinkService::get_link_stats(
        &state,
        &q.short_code,
        user.id,
        q.days,
    ).await?;

    Ok(Json(stats))
}

