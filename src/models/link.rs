use tracing::warn;
use redis::AsyncCommands;
use std::collections::HashMap;
use sqlx::{
    mysql::{MySql, MySqlDatabaseError, MySqlQueryResult}, prelude::FromRow, MySqlPool, QueryBuilder, Transaction
};
use deadpool_redis::Connection;
use axum::http::StatusCode;
use chrono::{
    DateTime,
    NaiveDateTime,
    Utc,
    Duration,
    TimeZone,
};
use chrono_tz::Tz;
use serde::{Serialize, Deserialize};

use crate::handlers::shortlink::LinkQuery;


#[derive(Debug, Default)]
struct VisitLog {
    short_code: String,
    long_url: String,
    ip: String,
    user_agent: String,
    referer: String,
    visit_time: String,
}


#[derive(FromRow, Debug, Serialize, Deserialize)]
pub struct LinkDto {
    pub id: u64,
    pub user_id: u64,
    pub short_code: String,
    pub long_url: String,
    pub click_count: u64,
    pub expire_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}


/// 只在返回 JSON 时使用
#[derive(Serialize, Deserialize)]
pub struct LinkView {
    pub id: u64,
    pub user_id: u64,
    pub short_code: String,
    pub long_url: String,
    pub click_count: u64,
    pub expire_at: Option<String>,
    pub created_at: String,
}


pub struct Link;

impl Link {
    /// 插入长 URL
    pub async fn insert_long_url(
        tx: &mut Transaction<'_, MySql>, 
        long_url: &str,
        expire_at: DateTime<Utc>,
        user_id: u64
    ) -> Result<MySqlQueryResult, (StatusCode, String)> {
        let insert_sql = sqlx::query(
            r#"INSERT INTO links (long_url, expire_at, user_id) VALUES (?, ?, ?)"#
        )
        .bind(long_url)
        .bind(expire_at)
        .bind(user_id)
        .execute(tx.as_mut())
        .await
        .map_err(
            |e| {
                warn!("insert_long_url: DB insert error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("DB insert error: {}", e))
            }
        )?;
    
        Ok(insert_sql)
    }

    /// 更新短码
    pub async fn update_short_code(
        tx: &mut Transaction<'_, MySql>, 
        id: u64,
        short_code: &str
    ) -> Result<(), (StatusCode, String)> {
        sqlx::query!(
            r#"UPDATE links SET short_code = ? WHERE id = ?"#,
            short_code,
            id,
        )
        .execute(tx.as_mut())
        .await
        .map_err(|e| {
            warn!("update_short_code: DB update error: {}", e);
            if let sqlx::Error::Database(db_err) = &e {
                if let Some(mysql_err) = db_err.try_downcast_ref::<MySqlDatabaseError>() {
                    // 1062 = Duplicate entry — violates UNIQUE constraint on short_code
                    if mysql_err.number() == 1062 {
                        return (StatusCode::CONFLICT, "Short code already exists".into());
                    }
                }
            }
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB update error: {}", e))
        })?;
    
        Ok(())
    }

    /// 设置短码
    pub async fn set_shortlink(
        redis_mgr: &mut Connection,
        short_code: &str,
        long_url: &str,
        ttl: i64,
    ) -> Result<(), (StatusCode, String)> {
        // 设置短链映射
        let url_key = format!("shortlink:{}", short_code);
        let _: () = redis_mgr.set_ex(&url_key, long_url, ttl as u64)
            .await
            .map_err(|e| {
                warn!("set_shortlink: Redis set_ex error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis set_ex error: {}", e))
            })?;
        
        Ok(())
    }

    /// 设置短码点击量
    pub async fn set_click_count(
        redis_mgr: &mut Connection,
        short_code: &str,
        click_ttl: i64,
    ) -> Result<(), (StatusCode, String)> {
        let click_key = format!("shortlink_click:{}", short_code);
        let _: () = redis::cmd("SET")
            .arg(&click_key)
            .arg(0)
            .arg("NX")
            .arg("EX")
            .arg(click_ttl)
            .query_async(redis_mgr)
            .await
            .map_err(|e| {
                warn!("set_click_count: Redis SET NX EX error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis SET NX EX error: {}", e))
            })?;
        
        Ok(())
    }

    /// 点击次数+1
    pub async fn in_click_count(
        redis_mgr: &mut Connection,
        short_code: &str,
    ) {
        let key = format!("shortlink_click:{}", short_code);
        let result: redis::RedisResult<i64> = redis_mgr
            .incr(&key, 1)
            .await;
            
        if let Err(e) = result {
            warn!("Redis INCR error: {} key={}", e, key);
        }
    }

    /// 记录访问
    pub async fn log_visit_to_stream(
        redis_mgr: &mut Connection,
        short_code: &str,
        long_url: &str,
        ip: &str,
        user_agent: &str,
        referer: &str,
    ) {
        let now = Utc::now().to_rfc3339();
        let result: redis::RedisResult<String> = redis_mgr.xadd(
            "visit_log", 
            "*", 
            &[
                ("short_code", short_code),
                ("long_url", long_url),
                ("ip", ip),
                ("user_agent", user_agent),
                ("referer", referer),
                ("visit_time", &now),
            ]
        )
        .await;
    
        if let Err(e) = result {
            warn!("log_visit_to_stream: Redis xadd error: {}", e);
        }
    }

    /// 从 Redis 获取长 URL
    pub async fn get_long_url_from_redis(
        redis_mgr: &mut Connection,
        short_code: &str,
    ) -> Result<Option<String>, (StatusCode, String)> {

        let key = format!("shortlink:{}", short_code);
        // 从 Redis 获取映射值
        let long_url: Option<String> = redis_mgr
            .get(&key)
            .await
            .map_err(|e| {
                warn!("get_long_url_from_redis: Redis get error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis get error: {}", e))
            })?;
        
        Ok(long_url)
    }

    /// 从 MySQL 获取长 URL
    pub async fn get_logn_url_from_mysql(
        mysql_pool: &MySqlPool,
        short_code: &str,
    ) -> Result<(String, Option<NaiveDateTime>), (StatusCode, String)> {
        let row = sqlx::query!(
            r#"SELECT long_url, expire_at FROM links WHERE short_code = ?"#,
            short_code,
        )
        .fetch_optional(mysql_pool)
        .await
        .map_err(|e| {
            warn!("get_logn_url_from_mysql: DB select error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB select error: {}", e))
        })?;
    
        match row {
            Some(row) => {
                Ok((row.long_url, row.expire_at))
            },
            None => {
                warn!("get_logn_url_from_mysql: 短码不存在: short_code={}", short_code);
                Err((StatusCode::NOT_FOUND, "Short code not found".into()))
            },
        }
    }

    /// 同步点击量
    pub async fn sync_click_counts(
        mysql_pool: &MySqlPool,
        redis_mgr: &mut Connection,
        batch: usize,
    ) -> Result<(), (StatusCode, String)> {
        let mut cursor: u64 = 0;

        loop {
            // 扫描 Redis 中的短码(100 个)
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("shortlink_click:*")
                .arg("COUNT")
                .arg(batch)
                .query_async(redis_mgr)
                .await
                .map_err(|e| {
                    warn!("sync_click_counts: Redis scan error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis scan error: {}", e))
                })?;

            // 遍历短码
            for key in keys {
                // 获取短码
                if let Some(code) = key.strip_prefix("shortlink_click:") {
                    // 获取短码点击量
                    let click_count: Option<i64> = redis_mgr
                        .get(&key)
                        .await
                        .map_err(|e| {
                            warn!("sync_click_counts: Redis get error: {} code={}", e, code);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR, 
                                format!("Redis get error: {}", e)
                            )
                        })?;

                    if let Some(click_count) = click_count {
                        // 如果点击量大于 0 更新 MySQL
                        if click_count > 0 {
                            sqlx::query!(
                                r#"UPDATE links SET click_count = click_count + ? WHERE short_code = ?"#,
                                click_count,
                                code,
                            )
                            .execute(mysql_pool)
                            .await
                            .map_err(|e| {
                                warn!("sync_click_counts: DB update error: {} code={}", e, code);
                                (StatusCode::INTERNAL_SERVER_ERROR, format!("DB update error: {}", e))
                            })?;
                            
                            // 将 Redis 点击量重置为 0
                            let _: () = redis_mgr
                                .set(&key, 0_i64)
                                .await
                                .map_err(|e| {
                                    warn!("sync_click_counts: Redis set error: {} code={}", e, code);
                                    (
                                        StatusCode::INTERNAL_SERVER_ERROR, 
                                        format!("Redis set error: {}", e)
                                    )
                                })?;
                        }
                    }
                }
            }

            // 如果没有短码了，退出循环
            if next_cursor == 0 {
                break;
            }
            cursor = next_cursor;
        }
        
        Ok(())
    }
    
    /// 同步访问日志
    pub async fn sync_visit_logs(
        mysql_pool: &MySqlPool,
        redis_mgr: &mut Connection,
        batch: usize,
    ) -> Result<(), (StatusCode, String)> {
        loop {
            // 1. 从 Stream 读出一批记录（XRANGE visit_log - + COUNT batch）
            //    返回值形如 Vec<(id, Vec<(field, value)>)>
            let entries: Vec<(String, Vec<(String, String)>)> = redis::cmd("XRANGE")
                .arg("visit_log")
                .arg("-")
                .arg("+")
                .arg("COUNT")
                .arg(batch)
                .query_async(redis_mgr)
                .await
                .map_err(|e| {
                    warn!("sync_visit_logs: Redis XRANGE error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis XRANGE error: {}", e))
                })?;

            // 若没有更多日志则结束
            if entries.is_empty() {
                break;
            }

            for (entry_id, kvs) in entries {
                // 2. 把字段映射到变量
                let mut visit_log = VisitLog::default();

                for (field, value) in kvs {
                    match field.as_str() {
                        "short_code"  => visit_log.short_code  = value,
                        "long_url"    => visit_log.long_url    = value,
                        "ip"          => visit_log.ip          = value,
                        "user_agent"  => visit_log.user_agent  = value,
                        "referer"     => visit_log.referer     = value,
                        "visit_time"  => visit_log.visit_time  = value,
                        _ => {}
                    }
                }

                // 3. 写入 MySQL
                sqlx::query!(
                    r#"INSERT INTO visit_logs
                       (short_code, long_url, ip, user_agent, referer, visit_time)
                       VALUES (?, ?, ?, ?, ?, ?)"#,
                    visit_log.short_code,
                    visit_log.long_url,
                    visit_log.ip,
                    visit_log.user_agent,
                    visit_log.referer,
                    visit_log.visit_time,
                )
                .execute(mysql_pool)
                .await
                .map_err(|e| {
                    warn!("sync_visit_logs: DB insert error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("DB insert error: {}", e))
                })?;

                // 4. 删除已同步的 Stream 条目，避免重复同步
                let _: () = redis::cmd("XDEL")
                    .arg("visit_log")
                    .arg(&entry_id)
                    .query_async(redis_mgr)
                    .await
                    .map_err(|e| {
                        warn!("sync_visit_logs: Redis XDEL error: {}", e);
                        (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis XDEL error: {}", e))
                    })?;
            }
        }

        Ok(())
    }

    /// 拼接 SQL 查询
    fn apply_filters<'a>(
        qb: &mut QueryBuilder<'a, MySql>,
        filter: &'a LinkQuery,
    ) {
        if let Some(user_id) = filter.user_id {
            qb.push(" AND user_id = ").push_bind(user_id);
        }
        
        if let Some(short_code) = filter.short_code.as_deref() {
            qb.push(" AND short_code LIKE ").push_bind(format!("%{}%", short_code));
        }

        if let Some(long_url) = filter.long_url.as_deref() {
            qb.push(" AND long_url LIKE ").push_bind(format!("%{}%", long_url));
        }
        if let Some(click_count) = filter.click_count {
            qb.push(" AND click_count = ").push_bind(click_count);
        }

        if let Some(date_from) = filter.date_from {
            qb.push(" AND created_at >= ").push_bind(date_from);
        }

        if let Some(date_to) = filter.date_to {
            qb.push(" AND created_at <= ").push_bind(date_to);
        }

        // 只查询未过期的短链（expire_at 为 NULL 或大于当前时间）
        qb.push(" AND (expire_at IS NULL OR expire_at > NOW())");
    }

    /// 构建返回数据
    fn to_view(src: LinkDto) -> LinkView {
        let fmt = "%Y-%m-%d %H:%M:%S";
        LinkView {
            id: src.id,
            user_id: src.user_id,
            short_code: src.short_code,
            long_url: src.long_url,
            click_count: src.click_count,
            expire_at: src.expire_at.map(|t| t.format(fmt).to_string()),
            created_at: src.created_at.format(fmt).to_string(),
        }
    }

    /// 查询短链列表
    pub async fn find_links(
        mysql_pool: &MySqlPool,
        filter: &LinkQuery,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<LinkView>, i64), (StatusCode, String)> {

        let mut data_qb: QueryBuilder<MySql> = QueryBuilder::new(
            "SELECT id, user_id, short_code, long_url, click_count, "
        );
        data_qb
            .push("CONVERT_TZ(expire_at, 'UTC', ")
            .push_bind(&filter.timezone)
            .push(") AS expire_at, ")
            .push("CONVERT_TZ(created_at, 'UTC', ")
            .push_bind(&filter.timezone)
            .push(") AS created_at FROM links WHERE 1 = 1 ");

        // 添加筛选条件
        Self::apply_filters(&mut data_qb, filter);

        // 分页 & 排序
        data_qb.push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);

        // 编译执行
        let rows = data_qb.build_query_as::<LinkDto>()
            .fetch_all(mysql_pool)
            .await
            .map_err(|e| {
                warn!("find_links: DB select error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("DB select error: {}", e))
            })?;

        let items = rows
            .into_iter()
            .map(Self::to_view)
            .collect();

        // 统计总数
        let mut count_qb: QueryBuilder<MySql> = QueryBuilder::new(
            "SELECT COUNT(*) FROM links WHERE 1 = 1 "
        );
        Self::apply_filters(&mut count_qb, filter);
        let count: i64 = count_qb.build_query_scalar()
            .fetch_one(mysql_pool)
            .await
            .map_err(|e| {
                warn!("find_links: DB select error (count): {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("DB select error: {}", e))
            })?;

        Ok((items, count))
    }

    /// 删除短链(手动)
    pub async fn delete_links(
        tx: &mut Transaction<'_, MySql>,
        redis_mgr: &mut Connection,
        link_ids: &[u64],
        user_id: u64,
    ) -> Result<(), (StatusCode, String)> {
        // 查询待删除记录的 short_code，后面删除 Redis 缓存
        let mut code_qb: QueryBuilder<MySql> = QueryBuilder::new(
            "SELECT short_code FROM links WHERE user_id = "
        );
        code_qb.push_bind(user_id)
              .push(" AND id IN (");
        let mut sep = code_qb.separated(", ");
        for id in link_ids {
            sep.push_bind(id);
        }
        code_qb.push(")");
        let short_codes: Vec<(String,)> = code_qb
            .build_query_as()
            .fetch_all(tx.as_mut())
            .await
            .map_err(
                |e| {
                    warn!("delete_links: DB select error: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR, 
                        format!("DB select error: {}", e)
                    )
                }
            )?;

        if !short_codes.is_empty() {
            // todo: 是否直接物理删除？visit_log 表中的记录是否需要保留(保留短链不会被回收)？
            // 暂时先直接删除
            // 构造并执行批量 DELETE
            let mut qb = QueryBuilder::new("DELETE FROM links WHERE id IN ( ");
            let mut separated = qb.separated(", ");
            for link_id in link_ids {
                separated.push_bind(link_id);
            }
            qb.push(") AND user_id = ").push_bind(user_id);
            qb.build().execute(tx.as_mut())
                .await
                .map_err(
                    |e| {
                        warn!("delete_links: DB Delete error: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR, 
                            format!("DB Delete error: {}", e)
                        )
                    }
                )?;
            
            // 将visit_log表中对应的短链删除
            let mut qb = QueryBuilder::new("DELETE FROM visit_logs WHERE short_code IN ( ");
            let mut separated = qb.separated(", ");
            for (short_code,) in &short_codes {
                separated.push_bind(short_code);
            }
            qb.push(")");
            qb.build().execute(tx.as_mut())
                .await
                .map_err(
                    |e| {
                        warn!("delete_links: DB Delete error: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR, 
                            format!("DB Delete error: {}", e)
                        )
                    }
                )?;

            // 构造并执行批量 UNLINK
            let mut pipe = redis::pipe();
            pipe.atomic();
            for (code,) in &short_codes {
                pipe.cmd("UNLINK").arg(format!("shortlink:{}", code)).ignore();
                pipe.cmd("UNLINK").arg(format!("shortlink_click:{}", code)).ignore();
            }
            let _: () = pipe.query_async(redis_mgr)
                .await
                .map_err(|e| {
                    warn!("delete_links: Redis unlink error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis unlink error: {}", e))
                })?;
        }

        Ok(())
    }

    /// 过期短链删除(定时任务)
    pub async fn delete_expired_links(
        mysql_pool: &MySqlPool,
    ) -> Result<(), (StatusCode, String)> {
        // 构造并执行批量 DELETE
        let mut qb = QueryBuilder::new(
            "DELETE FROM links WHERE expire_at < NOW()"
        );
        qb.build().execute(mysql_pool)
            .await
            .map_err(
                |e| {
                    warn!("delete_expired_links: DB Delete error: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR, 
                        format!("DB Delete error: {}", e)
                    )
                }
            )?;

        Ok(())
    }

    /// 点击量统计（按天）
    /// 返回一个按日期升序排列的 `(yyyy-mm-dd, 点击量)` 列表
    pub async fn count_daily_visits_by_code(
        mysql_pool: &MySqlPool,
        short_code: &str,
        timezone: String,
        user_id: u64,
        days: u8,
    ) -> Result<Vec<(String, i64)>, (StatusCode, String)> {
        // 校验短链归属
        let row = sqlx::query!(
            r#"SELECT id FROM links WHERE short_code = ? AND user_id = ?"#,
            short_code,
            user_id,
        )
        .fetch_optional(mysql_pool)
        .await
        .map_err(
            |e| {
                warn!("count_daily_visits_by_code: DB select error: {} short_code={} user_id={}", e, short_code, user_id);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("DB select error: {}", e))
            }
        )?;

        if row.is_none() {
            warn!("count_daily_visits_by_code: 短码不存在: short_code={} user_id={}", short_code, user_id);
            return Err((StatusCode::NOT_FOUND, "Short code not found".into()));
        }


        // 计算 UTC 查询范围
        let tz: Tz = timezone.parse().map_err(|_| {
            warn!("count_daily_visits_by_code: invalid timezone: {}", timezone);
            (StatusCode::BAD_REQUEST, "Invalid timezone".to_string())
        })?;
        let now_utc = Utc::now();
        let now_local = now_utc.with_timezone(&tz);

        // 本地起始 00:00:00（days 天 含当天）
        let start_local_midnight = (now_local - Duration::days(days as i64 - 1))
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let start_local_dt = tz
            .from_local_datetime(&start_local_midnight)
            .single()
            .ok_or_else(|| {
                warn!("count_daily_visits_by_code: ambiguous local datetime for start_midnight");
                (StatusCode::INTERNAL_SERVER_ERROR, "Ambiguous local datetime".to_string())
            })?;
        let start_utc = start_local_dt.naive_utc();
    
        let today_local = now_local.date_naive();
        let start_local_date = start_local_dt.date_naive();

        // 执行聚合查询
        let rows = sqlx::query!(
            r#"
            SELECT DATE(CONVERT_TZ(visit_time, 'UTC', ?)) AS day_local, COUNT(*) AS cnt
            FROM visit_logs
            WHERE short_code = ? AND visit_time >= ? AND visit_time <= ?
            GROUP BY day_local
            ORDER BY day_local
            "#,
            timezone,
            short_code,
            start_utc,
            now_utc
        )
        .fetch_all(mysql_pool)
        .await
        .map_err(|e| {
            warn!("count_daily_visits_by_code: DB select error (visit_logs): {} short_code={}", e, short_code);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB select error: {}", e))
        })?;

        // 把查询结果放进 HashMap，键直接用 "YYYY-MM-DD" 字符串
        let mut day_map: HashMap<String, i64> = HashMap::new();
        for row in rows {
            if let Some(day) = row.day_local {
                let key = day.format("%Y-%m-%d").to_string();
                day_map.insert(key, row.cnt);
            }
        }

        // 组装连续 days 天的数据，缺失的日期补 0
        let mut result = Vec::with_capacity(days as usize);
        let mut d = start_local_date;
        while d <= today_local {
            let key_str = d.format("%Y-%m-%d").to_string();
            let cnt = day_map.get(&key_str).copied().unwrap_or(0);
            result.push((key_str, cnt));
            d = d.succ_opt().unwrap(); // 下一天
        }

        Ok(result)
    }
}