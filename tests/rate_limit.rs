//! ⚠️ 本文件为限流相关集成测试，涉及全局状态（如 Redis 计数）。
//!     为避免与其他测试相互影响，**请单独运行本测试文件,且单个函数运测试**，例如：
//!         cargo test --test rate_limit
//!     不建议与其它集成测试一起批量运行，否则可能导致测试不稳定或误报。
//! 集成测试：主要验证限流相关逻辑

use reqwest::{Client, StatusCode};
use tokio::time::{sleep, Duration};
use std::env;
use tokio_shortlink::services::LoginResp;

mod common;

#[tokio::test]
async fn test_ip_rate_limit_blocks_after_threshold() {
    // ip 限流测试
    // 用登录接口测试，因为登录接口有 ip 限流
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let login_url = format!("http://{}/login", addr);
    // 获取 ip 限流参数
    let ip_rate_limit = env::var("IP_RATE_LIMIT")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(5);
    let ip_rate_limit_window = env::var("IP_RATE_LIMIT_WINDOW")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(20);

    let login_body = serde_json::json!({
        "email": "test2@example.com",
        "password": "password2",
    });

    // 快速发送请求，直到超过阈值
    eprintln!("[test-ip-rate-limit] 发送 {} 次请求", ip_rate_limit);
    for _ in 0..ip_rate_limit {
        client
            .post(&login_url)
            .json(&login_body)
            .send()
            .await
            .unwrap();
    }
    
    // 第 ip_rate_limit + 1 次请求应该被限流
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);

    // 等待窗口期结束
    eprintln!("[test-ip-rate-limit] 等待 {} 秒", ip_rate_limit_window);
    sleep(Duration::from_secs(ip_rate_limit_window as u64)).await;

    // 再次请求应该成功
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::OK);
}


#[tokio::test]
async fn test_user_rate_limit_blocks_after_threshold() {
    // 用户限流测试
    // 用短链列表接口测试，因为登录接口有用户限流
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let login_url = format!("http://{}/login", addr);
    let links_url = format!("http://{}/links", addr);
    
    // 获取用户限流参数
    let user_rate_limit = env::var("USER_RATE_LIMIT")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3);
    let user_rate_limit_window = env::var("USER_RATE_LIMIT_WINDOW")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(20);

    let login_body = serde_json::json!({
        "email": "test1@example.com",
        "password": "password1",
    });

    // 登录获取 token
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    let token = res.json::<LoginResp>().await.unwrap().token;

    // 快速发送请求，直到超过阈值
    eprintln!("[test-user-rate-limit] 发送 {} 次请求", user_rate_limit);
    for _ in 0..user_rate_limit {
        let res = client
            .get(&links_url)
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        eprintln!("[test-user-rate-limit] 状态码: {}", res.status());
    }
    
    // 第 user_rate_limit + 1 次请求应该被限流
    // 注意：如果连续快速执行两次本测试文件，可能因为上一次的限流窗口尚未过期，
    // 导致本次测试在未超限时提前被限流，出现测试失败。
    let res = client
        .get(&links_url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);

    // 等待窗口期结束
    eprintln!("[test-user-rate-limit] 等待 {} 秒", user_rate_limit_window);
    sleep(Duration::from_secs(user_rate_limit_window as u64)).await;

    // 再次请求应该成功
    let res = client
        .get(&links_url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::OK);
}


#[tokio::test]
async fn test_register_rate_limit_ip() {
    // IP 注册限流
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let register_url = format!("http://{}/register", addr);

    let ip_register_limit = env::var("IP_REGISTER_LIMIT")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3);
    let ip_register_ttl = env::var("IP_REGISTER_TTL")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(20);

    eprintln!("[test-register-rate-limit-ip] 发送 {} 次请求", ip_register_limit);
    for i in 0..ip_register_limit {
        let register_body = serde_json::json!({
            "nickname": format!("Ben{}", i),
            "password": "Ben123456",
            "email": format!("ben{}@example.com", i),
        });

        let res = client
            .post(&register_url)
            .json(&register_body)
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success());
    }

    let register_body = serde_json::json!({
        "nickname": "Ben",
        "password": "Ben123456",
        "email": "ben@example.com",
    });

    let res = client
        .post(&register_url)
        .json(&register_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS); 
    
    // 等待窗口期结束
    eprintln!("[test-register-rate-limit-ip] 等待 {} 秒", ip_register_ttl);
    sleep(Duration::from_secs(ip_register_ttl as u64)).await;
    
    // 再次请求应该成功
    let res = client
        .post(&register_url)
        .json(&register_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);   
}


#[tokio::test]
async fn test_login_rate_limit_ip() {
    // IP 登录失败限流
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let ip_user_login_fail_limit = env::var("IP_USER_LOGIN_FAIL_LIMIT")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3);
    let ip_user_login_fail_ttl = env::var("IP_USER_LOGIN_FAIL_TTL")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3);
    let login_url = format!("http://{}/login", addr);
    let client = Client::new();

    let login_body = serde_json::json!({
        "email": "test1@example.com",
        "password": "password1",
    });

    let login_body_error = serde_json::json!({
        "email": "test1@example.com",
        "password": "abcedfggai",
    });

    // 发送错误请求，直到超过阈值
    eprintln!("[test-login-failed] 发送 {} 次请求", ip_user_login_fail_limit);
    for _ in 0..ip_user_login_fail_limit {
        client
            .post(&login_url)
            .json(&login_body_error)
            .send()
            .await
            .unwrap();
    }
    
    // 第 ip_user_login_fail_limit + 1 次请求应该被限流
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);

    // 等待窗口期结束
    eprintln!("[test-login-rate-limit-ip] 等待 {} 秒", ip_user_login_fail_ttl);
    sleep(Duration::from_secs(ip_user_login_fail_ttl as u64)).await;
    
    // 再次请求应该成功
    let res = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);   
}