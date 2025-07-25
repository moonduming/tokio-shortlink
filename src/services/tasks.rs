use std::sync::Arc;
use tracing::{error, info};
use tokio::time::{interval, Duration};
use rand::{rng, seq::IndexedRandom};
use crate::state::AppState;
use crate::models::link::Link;

/// 点击量同步
pub async fn spawn_click_count_sync(state: Arc<AppState>) {
    tokio::spawn(async move {
        // 每 15 分钟同步一次点击量
        let mut ticker = interval(Duration::from_secs(900));
        loop {
            ticker.tick().await;
            info!("Syncing click counts start");

            // 随机选择一个 Redis 连接
            let manager = match state.managers.choose(&mut rng()) {
                Some(manager) => manager,
                None => {
                    error!("No Redis manager(click_count_sync)");
                    continue
                },
            };
            let mut conn = manager.lock().await;

            // 同步点击量
            if let Err(e) = Link::sync_click_counts(
                &state.mysql_pool, 
                &mut conn,
                100
            ).await {
                error!("Failed to sync click counts: {:?}", e);
            }

            info!("Synced click counts end");
        }
    });
}


/// 访问日志同步
pub async fn spawn_visit_log_sync(state: Arc<AppState>) {
    tokio::spawn(async move {
        // 每 20 分钟同步一次访问日志
        let mut ticker = interval(Duration::from_secs(1200));
        loop {
            ticker.tick().await;
            info!("Syncing visit logs start");

            // 随机选择一个 Redis 连接
            let manager = match state.managers.choose(&mut rng()) {
                Some(manager) => manager,
                None => {
                    error!("No Redis manager(vist_log_sync)");
                    continue
                },
            };
            let mut conn = manager.lock().await;

            // 同步访问日志
            if let Err(e) = Link::sync_visit_logs(
                &state.mysql_pool, 
                &mut conn,
                100
            ).await {
                error!("Failed to sync visit logs: {:?}", e);
            }

            info!("Synced visit logs end");
        }
    });
}


/// 过期短链删除
pub async fn spawn_expired_links_delete(state: Arc<AppState>) {
    tokio::spawn(async move {
        // 每 30 分钟同步一次过期短链
        let mut ticker = interval(Duration::from_secs(1800));
        loop {
            ticker.tick().await;
            info!("Syncing expired links start");

            // 删除过期短链
            if let Err(e) = Link::delete_expired_links(
                &state.mysql_pool, 
            ).await {
                error!("Failed to delete expired links: {:?}", e);
            }

            info!("Synced expired links end");
        }
    });
}
