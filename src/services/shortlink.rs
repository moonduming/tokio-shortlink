use rand::{rng, seq::IndexedRandom};
use axum::http::StatusCode;
use redis::aio::ConnectionManager;
use crate::{
    handlers::shortlink::LinkQuery, 
    models::link::{Link, LinkDto}, 
    state::AppState
};


pub struct ShortlinkService;

impl ShortlinkService {
    
    /// Base62 编码函数
    fn encode_base62(mut id: u64) -> String {
        const BASE62_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        let mut buf = Vec::new();
        while id > 0 {
            buf.push(BASE62_CHARS[(id % 62) as usize]);
            id /= 62;
        }
        if buf.is_empty() {
            buf.push(b'0');
        }
        buf.reverse();
        String::from_utf8(buf).unwrap()
    }

    /// 创建短链
    pub async fn create_shortlink(
        state: &AppState,
        long_url: &str,
        user_short_code: Option<String>,
        ttl: i64,
        user_id: u64
    ) -> Result<String, (StatusCode, String)> {
        let expire_at = chrono::Utc::now() + chrono::Duration::seconds(ttl);
        // 开启事务
        let mut tx = state
            .mysql_pool
            .begin()
            .await
            .map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("DB Begin error: {}", e))
            })?;

        // 插入长 URL
        let insert_sql = Link::insert_long_url(
            &mut tx, 
            long_url,
            expire_at,
            user_id
        ).await?;
    
        let id = insert_sql.last_insert_id();
        let mut short_code = String::new();

        if let Some(user_short_code) = user_short_code {
            short_code = user_short_code;

            // 直接尝试写入；若违反 UNIQUE 约束， update_short_code 会返回 CONFLICT
            match Link::update_short_code(&mut tx, id, &short_code).await {
                Ok(_) => {}
                Err((StatusCode::CONFLICT, _)) => {
                    // 用户自定义短码已存在
                    return Err((StatusCode::BAD_REQUEST, "Short code already exists".into()));
                }
                Err(e) => return Err(e),
            }
        } else {
            // 尝试最多 100 次自动生成；遇到唯一键冲突就换一个新码
            for i in 0..100 {
                let candidate = Self::encode_base62(id + i as u64);
                match Link::update_short_code(&mut tx, id, &candidate).await {
                    Ok(_) => {
                        short_code = candidate;
                        break;
                    }
                    Err((StatusCode::CONFLICT, _)) => continue, // 短码碰撞，重试
                    Err(e) => {
                        return Err(e);
                    }
                }
            }

            if short_code.is_empty() {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Unable to generate unique short code".into(),
                ));
            }
        }

        tx.commit().await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB Commit error: {}", e))
        })?;

        // 随机选择一个 Redis 连接
        let manager = state.managers
            .choose(&mut rng())
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into()))?;

        let mut conn = manager.lock().await;
    
        // 判断过期时间是否大于设置的redis最大存储时间
        // 大于则设置为最大存储时间
        let config = state.config.read().await;
        let redis_max_ttl = config.redis_max_ttl;
        let cache_ttl = if ttl > redis_max_ttl {
            redis_max_ttl
        } else {
            ttl
        };

        // 将短码和长 URL 存储到 Redis
        Link::set_shortlink(
            &mut conn,
            &short_code,
            long_url,
            cache_ttl,
        ).await?;

        // 设置点击量
        Link::set_click_count(
            &mut conn,
            &short_code,
            ttl,
        ).await?;
    
        let base = config.addr.clone();
        Ok(format!("{}/{}", base.trim_end_matches('/'), short_code))
    }

    /// 增加点击数和访问日志
    async fn push_click_and_log(
        redis_mgr: &mut ConnectionManager,
        short_code: &str,
        long_url: &str,
        ip: &str,
        user_agent: &str,
        referer: &str,
    ) {
        Link::log_visit_to_stream(
            redis_mgr,
            short_code,
            long_url,
            ip,
            user_agent,
            referer,
        ).await;
        
        Link::in_click_count(
            redis_mgr,
            short_code,
        ).await;
    }

    /// 获取长链
    pub async fn get_long_url(
        ip: &str,
        user_agent: &str,
        referer: &str,
        state: &AppState,
        short_code: &str,
    ) -> Result<String, (StatusCode, String)> {
        // 随机选择一个 Redis 连接
        let manager = state.managers
            .choose(&mut rng())
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into()))?;

        let mut conn = manager.lock().await;
        
        // redis 命中
        if let Some(long_url) = Link::get_long_url_from_redis(
            &mut conn, 
            short_code
        ).await? {
            Self::push_click_and_log(
                &mut conn,
                short_code,
                &long_url,
                ip,
                user_agent,
                referer,
            ).await;
            return Ok(long_url)
        }

        // MySQL 回溯
        let (long_url, expire_opt) = Link::get_logn_url_from_mysql(
            &state.mysql_pool, 
            short_code
        ).await?;

        // 有设置过期时间(None为永久)
        if let Some(expire) = expire_opt {
            let now_ts = chrono::Utc::now().timestamp();
            let ttl = expire.and_utc().timestamp() - now_ts;
            // 已过期
            if ttl <= 0 {
                return Err((StatusCode::NOT_FOUND, "Link expired".into()));
            }

            // 未过期，且剩余时间大于redis缓存最小剩余有效期
            if ttl > state.config.read().await.redis_min_cache_ttl {
                Link::set_shortlink(
                    &mut conn,
                    short_code,
                    &long_url,
                    ttl,
                ).await?;
            }
        }
        
        Self::push_click_and_log(
            &mut conn,
            short_code,
            &long_url,
            ip,
            user_agent,
            referer,
        ).await;

        Ok(long_url)
    }

    /// 获取短链列表
    pub async fn list_links(
        state: &AppState,
        filter: &LinkQuery,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<LinkDto>, i64), (StatusCode, String)> {
        let (links, count) = Link::find_links(
            &state.mysql_pool,
            filter,
            limit,
            offset,
        ).await?;
        Ok((links, count))
    }

    /// 删除短链
    pub async fn delete_links(
        state: &AppState,
        link_ids: Vec<u64>,
        user_id: u64,
    ) -> Result<(), (StatusCode, String)> {
        let manager = state.managers
            .choose(&mut rng())
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into()))?;

        let mut conn = manager.lock().await;
        Link::delete_links(
            &state.mysql_pool,
            &mut conn,
            &link_ids,
            user_id,
        ).await?;

        Ok(())
    }

    /// 点击量统计（按天）
    pub async fn get_link_stats(
        state: &AppState,
        short_code: &str,
        user_id: u64,
        days: u8,
    ) -> Result<Vec<(String, i64)>, (StatusCode, String)> {
        // 校验days 是否超过最大值
        let max_days = state.config.read().await.max_stats_days;
        
        if days > max_days {
            return Err((StatusCode::BAD_REQUEST, "Days exceeds maximum allowed".into()));
        }
        
        Link::count_daily_visits_by_code(
            &state.mysql_pool,
            short_code,
            user_id,
            days,
        ).await
    }
    
}
    
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_base62() {
        assert_eq!(ShortlinkService::encode_base62(1), "1");
        assert_eq!(ShortlinkService::encode_base62(62), "10");
        assert_eq!(ShortlinkService::encode_base62(62 * 62), "100");
    }
}
