use reqwest::{Client, StatusCode};
use serde_json;
use std::env;
use tokio_shortlink::services::LoginResp;

mod common;


#[tokio::test]
async fn test_create_shortlink_success() {
    // 创建短链成功
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    // 获取短链最长过期时间
    let shortlink_max_ttl = env::var("SHORTLINK_MAX_TTL")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3600);

    let create_url = format!("http://{}/shorten", addr);
    let login_url = format!("http://{}/login", addr);

    // 登录获取 token
    let login_body = serde_json::json!({
        "email": "test2@example.com",
        "password": "password2",
    });
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    
    let token = res.json::<LoginResp>().await.unwrap().token;
    
    // 创建短链
    let create_body = serde_json::json!({
        "url": "https://github.com/moonduming/tokio-shortlink#",
        "ttl": shortlink_max_ttl,
        "short_code": null
    });

    let create_body2 = serde_json::json!({
        "url": "https://github.com/moonduming/tokio-shortlink#",
        "ttl": shortlink_max_ttl,
        "short_code": "create"
    });

    let res = client
        .post(&create_url)
        .bearer_auth(&token)
        .json(&create_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    
    // 创建短链
    let res = client
        .post(&create_url)
        .bearer_auth(&token)
        .json(&create_body2)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}


#[tokio::test]
async fn test_create_shortlink_invalid_ttl() {
    // 创建短链失败，TTL 超出范围
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    // 获取短链最长过期时间
    let shortlink_max_ttl = env::var("SHORTLINK_MAX_TTL")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3600);
    // 获取短链最短过期时间
    let shortlink_min_ttl = env::var("SHORTLINK_MIN_TTL")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(60);
    let create_url = format!("http://{}/shorten", addr);
    let login_url = format!("http://{}/login", addr);

    // 登录获取 token
    let login_body = serde_json::json!({
        "email": "test2@example.com",
        "password": "password2",
    });
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    
    let token = res.json::<LoginResp>().await.unwrap().token;
    
    // 创建短链
    let create_body = serde_json::json!({
        "url": "https://github.com/moonduming/tokio-shortlink#",
        "ttl": shortlink_max_ttl + 1,
        "short_code": null
    });

    let create_body2 = serde_json::json!({
        "url": "https://github.com/moonduming/tokio-shortlink#",
        "ttl": shortlink_min_ttl - 1,
        "short_code": null
    });

    let res = client
        .post(&create_url)
        .bearer_auth(&token)
        .json(&create_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    
    let res = client
        .post(&create_url)
        .bearer_auth(&token)
        .json(&create_body2)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}


#[tokio::test]
async fn test_create_shortlink_invalid_short_code() {
    // 创建短链失败，短链码已存在
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    // 获取短链最长过期时间
    let shortlink_max_ttl = env::var("SHORTLINK_MAX_TTL")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3600);
    let create_url = format!("http://{}/shorten", addr);
    let login_url = format!("http://{}/login", addr);

    // 登录获取 token
    let login_body = serde_json::json!({
        "email": "test2@example.com",
        "password": "password2",
    });
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    
    let token = res.json::<LoginResp>().await.unwrap().token;
    
    // 创建短链
    let create_body = serde_json::json!({
        "url": "https://github.com/moonduming/tokio-shortlink#",
        "ttl": shortlink_max_ttl,
        "short_code": "create"
    });

    let res = client
        .post(&create_url)
        .bearer_auth(&token)
        .json(&create_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
