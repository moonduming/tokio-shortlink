//! 集成测试：主要验证 token 相关接口的鉴权流程
use reqwest::{Client, StatusCode, redirect};
use reqwest::header::LOCATION;
use std::env;
use tokio_shortlink::services::users::LoginResp;

mod common;


#[tokio::test]
async fn test_public_route_without_token_should_succeed() {
    // 测试公开接口（如注册、登录、短链跳转）不需要 token 能正常访问
    // NOTE:
    // reqwest 默认会跟随 3xx 重定向，这样我们就拿不到短链服务本身返回的状态码。
    // 这里用一个自定义 client（不跟随重定向）来捕获真实返回。
    let addr = env::var("ADDR").unwrap_or("127.0.0.1:3000".to_string());
    let client = Client::builder()
        .redirect(redirect::Policy::none())
        .build()
        .unwrap();

    let response = client
        .get(format!("http://{}/s/test", addr))
        // 有些服务端会依赖 UA，但对我们的短链一般无所谓；保留以便排查。
        .header("User-Agent", "tokio-shortlink-test/0.1")
        .send()
        .await
        .unwrap();

    // 服务当前返回 303
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    // 验证 Location 头是否存在（跳转地址）
    let location = response.headers().get(LOCATION).expect("missing Location header");
    let location_str = location.to_str().unwrap();
    assert!(!location_str.is_empty(), "Location header must not be empty");
}


#[tokio::test]
async fn test_protected_route_with_valid_token_should_succeed() {
    // 测试需要 token 的接口，带合法 token 能访问
    // TODO: 登录获取 token；当前仅验证未带 token 时应 401。
    let addr = env::var("ADDR").unwrap_or("127.0.0.1:3000".to_string());
    let client = Client::new();
    let response = client
        .get(format!("http://{}/links", addr))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "protected route should reject missing token");
}


#[tokio::test]
async fn test_token_rate_limit_should_block_after_exceeding_limit() {
    // 测试用户 Token 访问次数达到阈值后被限流
    // 限制同时只能存在3个 Token，超过后旧 Token 会被拒绝
    let addr = env::var("ADDR").unwrap_or("127.0.0.1:3000".to_string());
    let client = Client::new();

    // 登录(账户已经在common.rs中创建)
    let login_body = serde_json::json!({
        "email": "test1@example.com",
        "password": "password1",
    });
    let res = client
        .post(format!("http://{}/login", addr))
        .json(&login_body)
        .send()
        .await
        .unwrap();
    
    // 获取 token
    let token = res.json::<LoginResp>().await.unwrap().token;
    // 验证 token
    let response = client
        .get(format!("http://{}/links", addr))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    let user_token_limit = env::var("USER_TOKEN_LIMIT")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(3);

    // 验证 token 超出限制
    for _ in 0..user_token_limit {
        // 循环登录三次，挤掉旧 token
        let _ = client
            .post(format!("http://{}/login", addr))
            .json(&login_body)
            .send()
            .await
            .unwrap();
    }
    
    // token 超出限制
    let response = client
        .get(format!("http://{}/links", addr))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
