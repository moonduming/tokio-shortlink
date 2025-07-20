// 集成测试：主要验证登录相关逻辑
use reqwest::{Client, StatusCode};
use std::env;
use serde_json;

mod common;


#[tokio::test]
async fn test_login_success() {
   // 登录成功
   let client = Client::new();
   let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
   let login_url = format!("http://{}/login", addr);
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
   assert_eq!(res.status(), StatusCode::OK);
}


#[tokio::test]
async fn test_login_failed() {
    // 登录失败
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let login_url = format!("http://{}/login", addr);
    let client = Client::new();

    let login_body_error = serde_json::json!({
        "email": "test1@example.com",
        "password": "abcedfggai", // 错误密码
    });
    
    let res = client
        .post(&login_url)
        .json(&login_body_error)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
