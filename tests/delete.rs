use reqwest::{Client, StatusCode};
use std::env;
use serde_json::json;
use tokio_shortlink::handlers::shortlink::LinkList;

mod common;


#[tokio::test]
async fn test_delete_success() {
    // 测试成功删除
    let client = Client::new();
    let addr = env::var("ADDR").unwrap();
    
    // 登录获取 token
    let login_url = format!("http://{}/login", addr);
    let login_body = json!({
        "email": "test3@example.com",
        "password": "password3",
    });
    let token = common::login(&login_url, &login_body).await;
    
    // 创建短链
    let shorten_url = format!("http://{}/shorten", addr);
    let shorten_body = json!({
        "url": "https://www.example.com",
        "short_code": "test_delete",
    });
    common::shorten(&shorten_url, &shorten_body, &token).await;

    // 获取短链数据
    let links_url = format!("http://{}/links?short_code=test_delete", addr);
    let res = client
        .get(&links_url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let links = res.json::<LinkList>().await.unwrap();
    assert_eq!(links.links.len(), 1);
    assert_eq!(links.count, 1);

    let link_id = links.links[0].id;
    
    // 删除短链
    let delete_url = format!("http://{}/delete", addr);
    let delete_body = json!({
        "ids": [link_id],
    });
    let res = client
        .post(&delete_url)
        .bearer_auth(&token)
        .json(&delete_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}


#[tokio::test]
async fn test_delete_failed() {
    // 测试失败删除
    let client = Client::new();
    let addr = env::var("ADDR").unwrap();
    
    // 登录获取 token
    let login_url = format!("http://{}/login", addr);
    let login_body = json!({
        "email": "test3@example.com",
        "password": "password3",
    });
    let token = common::login(&login_url, &login_body).await;
    
    // 删除短链
    let delete_url = format!("http://{}/delete", addr);
    let delete_body = json!({
        "ids": [-1],
    });
    let res = client
        .post(&delete_url)
        .bearer_auth(&token)
        .json(&delete_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);  

    let delete_body = json!({
        "ids": [],
    }); 
    let res = client
        .post(&delete_url)
        .bearer_auth(&token)
        .json(&delete_body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);  
}


#[tokio::test]
async fn test_delete_unauthorized_link() {
    // 测试未授权删除
    let client = Client::new();
    let addr = env::var("ADDR").unwrap();
    let login_url = format!("http://{}/login", addr);
    let login_body = json!({
        "email": "test0@example.com",
        "password": "password0",
    });
    let login_body2 = json!({
        "email": "test1@example.com",
        "password": "password1",
    });
    let token = common::login(&login_url, &login_body).await;
    let token2 = common::login(&login_url, &login_body2).await;
    
    // 创建短链
    let shorten_url = format!("http://{}/shorten", addr);
    let shorten_body = json!({
        "url": "https://www.example.com",
        "short_code": "un_link",
    });
    common::shorten(&shorten_url, &shorten_body, &token).await;

    // 获取短链数据
    let links_url = format!("http://{}/links?short_code=un_link", addr);
    let res = client
        .get(&links_url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let links = res.json::<LinkList>().await.unwrap();
    let link_id = links.links[0].id;
    
    // 使用未授权 token 删除短链
    let delete_url = format!("http://{}/delete", addr);
    let delete_body = json!({
        "ids": [link_id],
    });
    let res = client
        .post(&delete_url)
        .bearer_auth(&token2)
        .json(&delete_body)
        .send()
        .await
        .unwrap();
    // 未授权短链不会删除但返回成功
    assert_eq!(res.status(), StatusCode::OK);  
    
    // 再次获取短链数据
    let res = client
        .get(&links_url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let links = res.json::<LinkList>().await.unwrap();
    assert_eq!(links.links[0].id, link_id);
}
