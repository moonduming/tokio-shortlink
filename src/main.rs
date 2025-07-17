use std::{sync::Arc, net::SocketAddr};

use axum::{routing::{get, post}, Router};
use tokio::{net::TcpListener, sync::{Mutex, RwLock}};
use tracing_subscriber::fmt::time::LocalTime;
use tower_http::trace::TraceLayer;

use tokio_shortlink::models::db;
use tokio_shortlink::config::AppConfig;
use tokio_shortlink::state::AppState;
use tokio_shortlink::handlers::{shortlink, users};
use tokio_shortlink::middleware::{jwt_auth, ip_rate_limiter, user_rate_limiter};
use tokio_shortlink::services::{
    spawn_click_count_sync, 
    spawn_visit_log_sync, 
    spawn_expired_links_delete
};


#[tokio::main]
async fn main() {
    // 初始化数据库连接
    let cfg = AppConfig::from_env().unwrap();
    let mysql_pool = db::new_mysql_pool(&cfg.database_url).await.unwrap();
    let redis = db::new_redis_client(&cfg.redis_url).await.unwrap();
    let addr = cfg.addr.clone();

    // 初始化全局日志（本地时区，RFC3339 格式）
    tracing_subscriber::fmt()
        .with_timer(LocalTime::rfc_3339())
        .init();

    // 初始化 4 条 Redis 连接并包进 Arc<Mutex<_>>
    let mut managers = Vec::new();
    for _ in 0..4 {
        let mgr = redis.get_connection_manager().await.unwrap();
        managers.push(Mutex::new(mgr));
    }

    let state = Arc::new(AppState {
        mysql_pool,
        managers,
        config: RwLock::new(cfg),
    });

    // 启动点击量同步任务
    spawn_click_count_sync(state.clone()).await;
    
    // 启动访问日志同步任务
    spawn_visit_log_sync(state.clone()).await;

    // 启动过期短链删除任务
    spawn_expired_links_delete(state.clone()).await;

    let public = Router::new()
        .route("/login", post(users::login))
        .route("/register", post(users::register))
        .route("/s/{short_code}", get(shortlink::redirect))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(), 
            ip_rate_limiter
        ));

    // 保护路由
    let protected = Router::new()
        .route("/shorten", post(shortlink::create))
        .route("/links", get(shortlink::list_links))
        .route("/delete", post(shortlink::delete_links))
        .route("/stats", get(shortlink::get_link_stats))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(), 
            user_rate_limiter
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(), 
            jwt_auth
        ));
    
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .merge(public)
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    
    let listener = TcpListener::bind(addr).await.unwrap();
    let make_svc = app
        .into_make_service_with_connect_info::<SocketAddr>();

    axum::serve(listener, make_svc).await.unwrap();
    
}
