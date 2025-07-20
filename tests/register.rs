//! ⚠️ 本文件为注册相关集成测试，涉及全局状态（如 Redis 计数）。
//!     为避免与其他测试相互影响，**请单独运行本测试文件**，例如：
//!         cargo test --test register
//!     不建议与其它集成测试一起批量运行，否则可能导致测试不稳定或误报。
//!
//!     特别注意：
//!     本文件中包含注册限流相关测试函数，这类测试函数可能会因为状态共享（如同一 IP 限制）
//!     对其他测试造成干扰，**请避免将它们与正常注册测试函数同时运行**。
//!
//! 集成测试：主要验证注册相关逻辑

use reqwest::{Client, StatusCode};
use std::env;
use serde_json;

mod common;

#[tokio::test]
async fn test_register_success() {
    // 注册成功
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let register_url = format!("http://{}/register", addr);
    let register_body = serde_json::json!({
        "nickname": "Anan",
        "password": "Anan123456",
        "email": "anan@example.com",
    });

    let res = client
        .post(&register_url)
        .json(&register_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}


#[tokio::test]
async fn test_register_failed() {
    // 注册失败
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let register_url = format!("http://{}/register", addr);
    // 用户已经注册
    let register_body = serde_json::json!({
        "nickname": "Anan",
        "password": "Anan123456",
        "email": "anan@example.com",
    });

    let res = client
        .post(&register_url)
        .json(&register_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 注册失败，邮箱格式错误
    let register_body = serde_json::json!({
        "nickname": "Anan2",
        "password": "Anan123456",
        "email": "anan2",
    });

    let res = client
        .post(&register_url)
        .json(&register_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
