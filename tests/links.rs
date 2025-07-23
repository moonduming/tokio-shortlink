use std::env;
use reqwest::{Client, StatusCode};
use serde_json::json;
use tokio_shortlink::handlers::shortlink::LinkList;

mod common;

#[tokio::test]
async fn test_list_links() {
    // 短链列表访问测试
    let client = Client::new();
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let login_url = format!("http://{}/login", addr);
    let shorten_url = format!("http://{}/shorten", addr);
    let list_url = format!("http://{}/links", addr);

    // 获取 token
    let login_body = json!({
        "email": "test3@example.com",
        "password": "password3",
    });
    let token = common::login(&login_url, &login_body).await;

    // 创建短链
    let shorten_body = json!({
        "url": "https://www.example.com",
        "short_code": "list0",
    });
    let shorten_body2 = json!({
        "url": "https://www.example.com",
        "short_code": "list1",
    });
    common::shorten(&shorten_url, &shorten_body, &token).await;
    common::shorten(&shorten_url, &shorten_body2, &token).await;

    let res = client
        .get(&list_url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let links = res.json::<LinkList>().await.unwrap();
    assert_eq!(links.links.len(), 2);
    assert_eq!(links.count, 2);

    // 带参数
    let res = client
        .get(&list_url)
        .bearer_auth(&token)
        .query(&json!({
            "short_code": "list0",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let links = res.json::<LinkList>().await.unwrap();
    assert_eq!(links.links.len(), 1);
    assert_eq!(links.count, 1);
}
