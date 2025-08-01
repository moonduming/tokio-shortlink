use axum::{
    body::Body, 
    extract::{State, ConnectInfo}, 
    http::{Request, StatusCode}, 
    middleware::Next, 
    response::Response
};
use tracing::warn;
use std::{sync::Arc, net::SocketAddr};
use crate::state::AppState;
use redis::AsyncCommands;


pub async fn ip_rate_limiter(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let ip: String = addr.ip().to_string();
    // TODO: 当前限流策略仅基于 IP 地址，存在以下缺陷：
    // - 多用户共用同一个公网 IP（如校园网、公司网络、NAT 4G）时，某用户恶意请求将导致其他正常用户被误伤。
    // - 攻击者可使用代理/轮换 IP 绕过限流。
    // 可考虑的改进方式：
    // - 引入 Cookie ID 或 UA 指纹，辅助区分同一 IP 下不同用户。
    // - 对登录/注册等敏感接口引入行为验证码（如 hCaptcha、滑块）或账号维度限流。
    // - 限流维度多样化，如 IP + Path，或账号 + 失败计数。
    let key = format!("rate_limit:ip:{}", ip);
    // 从配置中读取限流参数
    let (limit, window_secs) = {
        let config = state.config.read().await;
        (config.ip_rate_limit, config.ip_rate_limit_window)
    };
    
    // redis 提前释放
    {
        // 获取redis连接
        let mut conn = state.redis_pool.get().await.map_err(|e| {
            warn!("ip_rate_limiter: Redis 获取连接失败: err={}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis err: {}", e))
        })?;

        // 限流逻辑
        let count: i64 = conn
            .incr(&key, 1)
            .await
            .map_err(|e| {
                warn!("ip_rate_limiter: Redis Incr 失败, ip={}, err={}", ip, e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Incr err: {}", e))
            })?;

        if count == 1 {
            // 第一次请求，设置过期时间
            let _: () = conn.expire(&key, window_secs)
                .await
                .map_err(|e| {
                    warn!("ip_rate_limiter: Redis Expire 失败, ip={}, err={}", ip, e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis Expire err: {}", e))
                })?;
        }
        
        if count > limit {
            warn!("ip_rate_limiter: 访问超限, ip={}, limit={}, window={}", ip, limit, window_secs);
            // 超出限制
            return Err((StatusCode::TOO_MANY_REQUESTS, "Too many requests".into()));
        }
    }
    
    Ok(next.run(req).await)
}