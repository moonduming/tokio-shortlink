use std::env;
use reqwest::{Client, StatusCode};
use serde_json::json;
use chrono::Utc;

mod common;

#[tokio::test]
async fn test_get_link_stats() {
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let login_url = format!("http://{}/login", addr);
    let shorten_url = format!("http://{}/shorten", addr);
    let stats_url = format!("http://{}/stats", addr);

    // 获取 token
    let login_body = json!({
    "email": "test3@example.com",
    "password": "password3",
    });
    let token = common::login(&login_url, &login_body).await;

    // 创建短链
    let shorten_body = json!({
        "url": "https://www.example.com",
        "short_code": "stats0",
    });
    common::shorten(&shorten_url, &shorten_body, &token).await; 

    let stats = client
        .get(&stats_url)
        .bearer_auth(&token)
        .query(&json!({
            "short_code": "stats0",
            "days": 1,
            "timezone": "Asia/Shanghai"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stats.status(), StatusCode::OK);

    let stats = stats.json::<Vec<(String, i64)>>().await.unwrap();
    // // 获取当前 UTC 日期，格式 YYYY-MM-DD
    let expected_date = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    assert_eq!(stats[0].0, expected_date);
}
