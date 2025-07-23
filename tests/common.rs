//! 测试公共初始化：加载 `.env.test` 并确保测试用户存在。
use std::env;
use reqwest::{Client, StatusCode};
use argon2::Argon2;
use password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
use sqlx::MySqlPool;
use tokio::runtime::Runtime;
use serde_json::Value;
use tokio_shortlink::services::LoginResp;

#[ctor::ctor]
fn init() {
    // 加载测试环境变量
    let _ = dotenvy::from_filename(".env.test");
    // 获取数据库 url
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not found");

    // 启动临时 tokio 运行时并执行异步 SQL
    Runtime::new().expect("create runtime").block_on(async {
        let pool = MySqlPool::connect(&db_url)
            .await
            .expect("connect to db");
        
        for i in 0..4 {
            let salt = SaltString::generate(&mut OsRng);
            let password = format!("password{}", i);
            let argon2 = Argon2::default();
            let hashed_pwd = argon2
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string();
            // 如果表有唯一索引(email) 可直接 INSERT ... ON DUPLICATE KEY UPDATE
            sqlx::query!(
                r#"INSERT IGNORE INTO users (nickname,email,password,status)
                    VALUES (?,?,?,1)"#,
                format!("test{}", i),
                format!("test{}@example.com", i),
                hashed_pwd     // 视实际字段做 hash
            )
            .execute(&pool)
            .await
            .expect("insert user");
        }
        
        // === 创建测试短链 ===
        let ttl: i64 = env::var("SHORTLINK_MAX_TTL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600);

        // 假设 links 表有 (short_code,url,ttl,expire_at,owner_id)
        // 这里 owner_id 用第 0 个用户
        sqlx::query!(
            r#"INSERT IGNORE INTO links (user_id, short_code, long_url, expire_at)
            VALUES (1, 'test', 'https://www.example.com', NOW() + INTERVAL ? SECOND)"#,
            ttl
        )
        .execute(&pool)
        .await
        .expect("insert link");
        });
    
}


#[allow(dead_code)]
pub async fn login(
    url: &str,
    body: &Value,
) -> String {
    let client = Client::new();
    let res = client
        .post(url)
        .json(&body)
        .send()
        .await
        .expect("login failed");
    assert_eq!(res.status(), StatusCode::OK);

    res.json::<LoginResp>().await.unwrap().token
}


#[allow(dead_code)]
pub async fn shorten(
    url: &str,
    body: &Value,
    token: &str,
){
    let client = Client::new();
    let res = client
        .post(url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .expect("shorten failed");
    assert_eq!(res.status(), StatusCode::OK);
}
