use std::{sync::Arc, net::SocketAddr, time::Duration};

use axum::{
    routing::{get, post}, 
    Router,
};
use tokio::sync::mpsc::channel;
use dashmap::DashSet;
use tokio::{net::TcpListener, sync::RwLock};
use tracing_subscriber::{fmt::time::LocalTime, EnvFilter};
use tower_http::{
    trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse},
    LatencyUnit,
    timeout::TimeoutLayer,
};
use tracing::Level;

use tokio_shortlink::models::db;
use tokio_shortlink::config::AppConfig;
use tokio_shortlink::state::AppState;
use tokio_shortlink::handlers::{shortlink, users};
use tokio_shortlink::middleware::{jwt_auth, ip_rate_limiter, user_rate_limiter};
use tokio_shortlink::services::{
    spawn_click_count_sync, 
    spawn_visit_log_sync, 
    spawn_expired_links_delete,
    background_jobs::{spawn_redis_workers, BackgroundJob},
};


#[tokio::main]
async fn main() {
    // 初始化配置
    let cfg = AppConfig::from_env().unwrap();

    // 初始化全局日志（本地时区，RFC3339 格式）
    tracing_subscriber::fmt()
        .with_timer(LocalTime::rfc_3339())
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    
    // 初始化数据库连接
    let mysql_pool = db::new_mysql_pool(
        &cfg.database_url,
        cfg.mysql_max_connections,
        cfg.mysql_acquire_timeout_ms,
        cfg.mysql_query_timeout_ms,
        cfg.mysql_lock_wait_timeout_s,
    ).await.unwrap();
    let redis_pool = db::new_redis_pool(
        &cfg.redis_url,
        cfg.redis_pool_size,
        cfg.redis_timeout_wait_ms,
        cfg.redis_timeout_create_ms,
        cfg.redis_timeout_recycle_ms,
    ).unwrap();

    let addr = cfg.addr.clone();
    // 全局超时层
    let timeout_layer = TimeoutLayer::new(Duration::from_millis(cfg.global_timeout_ms));

    // 构建管道
    let (tx, rx) = channel::<BackgroundJob>(cfg.bg_redis_queue_cap);
    let bg_redis_max_concurrency = cfg.bg_redis_max_concurrency;

    let state = Arc::new(AppState {
        mysql_pool,
        redis_pool,
        bg_redis_tx: tx.clone(),
        config: RwLock::new(cfg),
        pending_set: DashSet::new(),
    });

    spawn_redis_workers(
        state.clone(),
        rx,
        bg_redis_max_concurrency,
    );

    // 启动点击量同步任务
    spawn_click_count_sync(state.clone()).await;
    // 启动访问日志同步任务
    spawn_visit_log_sync(state.clone()).await;
    // 启动过期短链删除任务
    spawn_expired_links_delete(state.clone()).await;

    // Configure TraceLayer to log at INFO (defaults are DEBUG)
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(Level::INFO) // ensure method/path are recorded on the span at INFO
        )
        .on_request(())
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Millis)
        );

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
        .layer(trace_layer)
        .layer(timeout_layer)
        .with_state(state);
    
    // 启动服务
    let listener = TcpListener::bind(addr).await.unwrap();
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.expect("failed to install CTRL+C signal handler");
    };
    let make_svc = app
        .into_make_service_with_connect_info::<SocketAddr>();

    axum::serve(listener, make_svc)
    .with_graceful_shutdown(shutdown_signal)
    .await.unwrap();
    
}
