use axum::http::StatusCode;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::{rng, seq::IndexedRandom};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::{
    state::AppState, 
    models::user::User, 
    models::session::create_session
};


#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: u64,  // user id
    pub exp: i64, // 过期时间(Unix 秒)
    pub jti: String, // JWT ID
}


#[derive(Serialize, Deserialize)]
pub struct LoginResp {
    pub token: String,
    pub nickname: Option<String>,
}

pub struct UserService;

impl UserService {
    /// 注册
    pub async fn register(
        state: &AppState,
        nickname: &str,
        password: &str,
        email: &str,
        ip: &str,
    ) -> Result<(), (StatusCode, String)> {
        let manager = state
            .managers
            .choose(&mut rng()).ok_or(
                (StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into())
            )?;

        let mut conn = manager.lock().await;
        // 判断 IP 是否到达注册次数上限
        let config = state.config.read().await;
        let ip_register_key = format!("register:ip:{}", ip);
        let ip_register_limit = config.ip_register_limit;
        let ip_register_ttl = config.ip_register_ttl;

        User::can_register(&mut conn, ip_register_limit, &ip_register_key).await?;
        
        // 判断邮箱是否已经注册
        if User::exists_by_email(&state.mysql_pool, email).await? {
            return Err((StatusCode::BAD_REQUEST, "Email already registered".into()));
        }
        // 生成随机盐加密密码
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hashed_pwd = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR, 
                    format!("Password encryption failed: {}", e)
                )
            })?
            .to_string();

        // 记录注册次数
        User::record_register(&mut conn, &ip_register_key, ip_register_ttl).await?;
        
        User::create(&state.mysql_pool, nickname, &hashed_pwd, email).await?;

        Ok(())
    }

    pub async fn login(
        state: &AppState,
        email: &str,
        password: &str,
        ip: &str,
    ) -> Result<LoginResp, (StatusCode, String)> {
        // 根据邮箱查询用户
        let user = match User::find_user(&state.mysql_pool, None, Some(email)).await? {
            Some(user) => user,
            None => return Err((StatusCode::NOT_FOUND, "User not found".into())),
        };

        let manager = state.managers
            .choose(&mut rng())
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into()))?;

        let mut conn = manager.lock().await;
        
        let config = state.config.read().await;

        let user_login_fail_limit = config.user_login_fail_limit;
        let ip_user_login_fail_limit = config.ip_user_login_fail_limit;

        let user_fail_key = format!("login_fail:uid:{}", user.id);
        let ip_user_fail_key = format!("login_fail:ip_uid:{}:{}", ip, user.id);

        // 判断用户是否可以登录
        User::can_login(
            &mut conn,
            user_login_fail_limit,
            ip_user_login_fail_limit,
            &user_fail_key,
            &ip_user_fail_key,
        )
        .await?;

        // 验证密码 (argon2)
        let parsed_hash = PasswordHash::new(&user.password)
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR, 
                    "Password hash parse failed".into()
                )
            })?;

        let argon2 = Argon2::default();
        // 验证密码失败时记录失败并返回
        if let Err(_) = argon2.verify_password(password.as_bytes(), &parsed_hash) {
            let user_login_fail_ttl = config.user_login_fail_ttl;
            let ip_user_login_fail_ttl = config.ip_user_login_fail_ttl;
            User::record_login_fail(
                &mut conn,
                &user_fail_key,
                &ip_user_fail_key,
                user_login_fail_ttl,
                ip_user_login_fail_ttl,
            )
            .await?;
            return Err((StatusCode::UNAUTHORIZED, "Invalid password".into()));
        }

        let ttl = config.user_token_ttl;

        // 生成 JWT (有效期 1 天)
        let exp = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(ttl))
            .unwrap()
            .timestamp();

        let jti = Uuid::new_v4().to_string();

        // 保存 JWT ID 到 redis
        create_session(
            user.id,
            ttl, 
            &jti,
            &mut conn,
        )
        .await?;
        
        let claims = Claims { 
            sub: user.id, 
            exp, 
            jti: jti
        };
        
        let token = encode(
            &Header::default(), 
            &claims, 
            &EncodingKey::from_secret(config.jwt_secret.as_bytes())
        )
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("JWT err: {}", e))
        })?;

        User::login_success(
            &mut conn,
            &user_fail_key,
            &ip_user_fail_key,
        )
        .await?;
        
        Ok(LoginResp {
            token,
            nickname: user.nickname,
        })
    }
}