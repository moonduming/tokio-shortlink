use sqlx::MySqlPool;
use tokio::sync::{Mutex, RwLock};
use redis::aio::ConnectionManager;
use crate::config::AppConfig;

pub struct AppState {
    pub mysql_pool: MySqlPool,
    pub managers: Vec<Mutex<ConnectionManager>>,
    pub config: RwLock<AppConfig>,
}