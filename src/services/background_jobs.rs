use std::sync::Arc;
use tracing::{warn, info};
use tokio::sync::{mpsc::Receiver, Semaphore};
use crate::{
    models::link::Link,
    services::shortlink::ShortlinkService,
    state::{AppState, ScheduledJobKind},
};

/// 丢给后台的作业类型
#[derive(Debug)]
pub enum BackgroundJob {
    /// 推送点击量和访问日志
    PushClickAndLog {
        short_code: String,
        long_url: String,
        ip: String,
        user_agent: String,
        referer: String,
    },
    /// 设置点击量和缓存
    SetClickCount {
        short_code: String,
        long_url: String,
        cache_ttl: i64,
    },
    /// 启动点击量同步
    SpawnClickCountSync,
    /// 启动访问日志同步
    SpawnVisitLogSync,
    /// 启动过期短链删除
    SpawnExpiredLinksDelete,
}


/// 启动后台“固定并发 N + 有界队列”，返回用于投递作业的 tx
pub fn spawn_redis_workers(
    state: Arc<AppState>,
    mut rx: Receiver<BackgroundJob>,
    max_concurrency: usize,
) {
    let sem = Arc::new(Semaphore::new(max_concurrency));

    // 一个调度任务：串行从队列取活，按最多 N 并发派发
    tokio::spawn({
        async move {
            while let Some(job) = rx.recv().await {
                let state = state.clone();
                // 限制同时活跃任务数
                let sem = sem.clone();
                let permit = sem
                    .acquire_owned()
                    .await
                    .expect("semaphore closed");

                tokio::spawn(async move {
                    let _permit = permit;
                    // 每个作业自己从池里取连接；失败就告警返回
                    let mut conn = match state.redis_pool.get().await {
                        Ok(c) => c,
                        Err(e) => {
                            warn!("bg_redis: redis_pool.get() failed: {e}");
                            return;
                        }
                    };
                    match job {
                        BackgroundJob::PushClickAndLog { // 推送点击量和访问日志
                            short_code, 
                            long_url, 
                            ip, 
                            user_agent, 
                            referer 
                        } => {
                            ShortlinkService::push_click_and_log(
                                &mut conn, 
                                short_code, 
                                long_url, 
                                ip, 
                                user_agent, 
                                referer
                            ).await;
                        },
                        BackgroundJob::SetClickCount { // 设置点击量和缓存
                            short_code, 
                            long_url, 
                            cache_ttl 
                        } => {
                            if let Err(e) = Link::set_shortlink(
                                &mut conn,
                                &short_code,
                                &long_url,
                                cache_ttl,
                            ).await {
                                warn!("create_shortlink: Redis set_shortlink error: {:?}", e);
                            }
            
                            // 设置点击量
                            if let Err(e) = Link::set_click_count(
                                &mut conn,
                                &short_code,
                                cache_ttl,
                            ).await {
                                warn!("create_shortlink: Redis set_click_count error: {:?}", e);
                            }
                        },
                        BackgroundJob::SpawnClickCountSync => { // 启动点击量同步
                            info!("Syncing click counts start");
                            if let Err(e) = Link::sync_click_counts(
                                &state.mysql_pool, 
                                &mut conn,
                                100
                            ).await {
                                warn!("Failed to sync click counts: {:?}", e);
                            }
                            state.pending_set.remove(&ScheduledJobKind::SyncClick);
                            info!("Synced click counts end");
                        },
                        BackgroundJob::SpawnVisitLogSync => { // 启动访问日志同步
                            info!("Syncing visit logs start");
                            if let Err(e) = Link::sync_visit_logs(
                                &state.mysql_pool, 
                                &mut conn,
                                100
                            ).await {
                                warn!("Failed to sync visit logs: {:?}", e);
                            }
                            state.pending_set.remove(&ScheduledJobKind::SyncVisitLog);
                            info!("Synced visit logs end");
                        },
                        BackgroundJob::SpawnExpiredLinksDelete => { // 启动过期短链删除
                            info!("Syncing expired links start");
                            if let Err(e) = Link::delete_expired_links(
                                &state.mysql_pool, 
                            ).await {
                                warn!("Failed to delete expired links: {:?}", e);
                            }
                            state.pending_set.remove(&ScheduledJobKind::DeleteExpired);
                            info!("Synced expired links end");
                        },
                    };
                });
            }
        }
    });
}