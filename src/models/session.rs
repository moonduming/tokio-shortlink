use redis::Script;
use redis::aio::ConnectionManager;
use axum::http::StatusCode;


pub async fn create_session(
    user_id: u64,
    expire_secs: i64,
    jti: &str,
    redis_mgr: &mut ConnectionManager,
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
        if len > 3 then
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
        .invoke_async::<i32>(redis_mgr)
        .await
        .map_err(
            |e| 
            (
                StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Redis error: {}", e)
            )
        )?;

    Ok(())
}