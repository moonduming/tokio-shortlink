use redis::Script;
use deadpool_redis::Connection;
use axum::http::StatusCode;
use tracing::warn;


pub async fn create_session(
    user_token_limit: u8,
    user_id: u64,
    expire_secs: i64,
    jti: &str,
    redis_mgr: &mut Connection,
) -> Result<(), (StatusCode, String)> {
    // Redis key 名称
    let jti_key = format!("session:{}", jti);
    let list_key = format!("user_sessions:{}", user_id);

    // Lua 脚本
    // 存 jti 并将其写入 user_sessions 列表
    // 如果列表长度大于 3，删除最早的 jti
    let script = Script::new(r#"
        redis.call('SET', KEYS[1], 1, 'EX', ARGV[1])
        redis.call('RPUSH', KEYS[2], ARGV[2])

        local len = redis.call('LLEN', KEYS[2])
        if len > tonumber(ARGV[3]) then
            local old_jti = redis.call('LPOP', KEYS[2])
            if old_jti then
                redis.call('DEL', 'session:' .. old_jti)
            end
        end
        return 1
    "#);

    let _ = script
        .key(jti_key)
        .key(list_key)
        .arg(expire_secs)
        .arg(jti)
        .arg(user_token_limit)
        .invoke_async::<i32>(redis_mgr)
        .await
        .map_err(
            |e| {
                warn!(
                    "create_session: Redis 调用失败: user_id={}, jti={}, err={}",
                    user_id, jti, e
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR, 
                    format!("Redis error: {}", e)
                )
            }
        )?;

    Ok(())
}