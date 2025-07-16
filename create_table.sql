CREATE TABLE links (
  id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id         BIGINT UNSIGNED NOT NULL,            -- 用户ID，外键
  short_code      VARCHAR(16)     DEFAULT NULL,        -- 允许为空
  long_url        TEXT            NOT NULL,
  created_at      TIMESTAMP       NOT NULL DEFAULT CURRENT_TIMESTAMP,
  expire_at       DATETIME        NULL,
  click_count     BIGINT UNSIGNED NOT NULL DEFAULT 0,
  PRIMARY KEY (id),
  UNIQUE KEY uk_short (short_code),
  INDEX idx_user (user_id),                            -- 用户ID索引
  INDEX idx_created (created_at),
  CONSTRAINT fk_links_user FOREIGN KEY (user_id) REFERENCES users(id)
      ON DELETE CASCADE
      ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;


CREATE TABLE visit_logs (
  id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  short_code VARCHAR(16) NOT NULL,
  long_url TEXT NOT NULL,
  ip VARCHAR(45) NOT NULL,
  user_agent TEXT,
  referer TEXT,
  visit_time DATETIME NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;


CREATE TABLE users (
    id           BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    email        VARCHAR(128) NOT NULL UNIQUE COMMENT '邮箱',
    password     VARCHAR(128) NOT NULL COMMENT '密码hash, 务必加密存储',
    nickname     VARCHAR(32)  DEFAULT NULL COMMENT '昵称，可选',
    created_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '注册时间',
    updated_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
    status       TINYINT      NOT NULL DEFAULT 1 COMMENT '账号状态, 1=正常, 0=禁用'
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;