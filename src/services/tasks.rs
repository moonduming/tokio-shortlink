use std::sync::Arc;
use tokio::time::{interval, Duration};
use crate::state::{AppState, ScheduledJobKind};
use crate::services::background_jobs::BackgroundJob;
use tracing::warn;

/// 点击量同步
pub async fn spawn_click_count_sync(state: Arc<AppState>) {
    tokio::spawn(async move {
        // 从配置中读取点击量同步间隔
        let t = state.config.read().await.bg_click_counts_sync_interval;
        let mut ticker = interval(Duration::from_secs(t));
        loop {
            ticker.tick().await;
            if !state.pending_set.insert(ScheduledJobKind::SyncClick) {
                continue;
            }

            if let Err(e) = state.bg_redis_tx
                .try_send(BackgroundJob::SpawnClickCountSync) {
                state.pending_set.remove(&ScheduledJobKind::SyncClick);
                warn!("spawn_click_count_sync: bg_redis_tx try_send failed: {e}");
            }
        }
    });
}


/// 访问日志同步
pub async fn spawn_visit_log_sync(state: Arc<AppState>) {
    tokio::spawn(async move {
        // 从配置中读取访问日志同步间隔
        let t = state.config.read().await.bg_visit_logs_sync_interval;
        let mut ticker = interval(Duration::from_secs(t));
        loop {
            ticker.tick().await;
            if !state.pending_set.insert(ScheduledJobKind::SyncVisitLog) {
                continue;
            }

            if let Err(e) = state.bg_redis_tx
                .try_send(BackgroundJob::SpawnVisitLogSync) {
                state.pending_set.remove(&ScheduledJobKind::SyncVisitLog);
                warn!("spawn_visit_log_sync: bg_redis_tx try_send failed: {e}");
            }
        }
    });
}


/// 过期短链删除
pub async fn spawn_expired_links_delete(state: Arc<AppState>) {
    tokio::spawn(async move {
        // 从配置中读取过期短链删除间隔
        let t = state.config.read().await.bg_expired_links_sync_interval;
        let mut ticker = interval(Duration::from_secs(t));
        loop {
            ticker.tick().await;
            if !state.pending_set.insert(ScheduledJobKind::DeleteExpired) {
                continue;
            }

            if let Err(e) = state.bg_redis_tx
            .try_send(BackgroundJob::SpawnExpiredLinksDelete) {
                state.pending_set.remove(&ScheduledJobKind::DeleteExpired);
                warn!("spawn_expired_links_delete: bg_redis_tx try_send failed: {e}");
            }
        }
    });
}
