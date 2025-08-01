use sqlx::MySqlPool;
use tokio::sync::RwLock;
use deadpool_redis::Pool;
use crate::config::AppConfig;
use crate::services::background_jobs::BackgroundJob;
use tokio::sync::mpsc::Sender;
use dashmap::DashSet;


#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum ScheduledJobKind {
    SyncClick, 
    SyncVisitLog, 
    DeleteExpired
}


pub struct AppState {
    pub mysql_pool: MySqlPool,
    pub redis_pool: Pool,
    pub bg_redis_tx: Sender<BackgroundJob>,
    pub config: RwLock<AppConfig>,
    pub pending_set: DashSet<ScheduledJobKind>,
}