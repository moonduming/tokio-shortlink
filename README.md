# tokio-shortlink

A high-performance short link service built with Rust and Tokio.

## 简介

tokio-shortlink 是一个使用 Rust、Axum、SQLx 和 Redis 构建的短链接服务。它支持短链的创建、跳转统计、访问限制等功能，并采用 Tokio 异步运行时保证高并发性能。

> **重点提示：**  
> 在运行项目之前，请复制配置模板文件并进行自定义配置：  
> ```bash
> cp .env.example .env
> ```  
> 然后根据 `.env.example` 模板编辑 `.env` 文件，填写您的数据库连接、Redis 地址、JWT 密钥等信息。

## 特性

- 基于 Tokio 异步运行时
- 使用 Axum 搭建 HTTP API
- SQLx MySQL 作为持久层
- Redis 用于限流和统计缓存
- 支持 IP 和用户级限流
- JWT 认证

## 快速开始

1. 克隆仓库  
   ```bash
   git clone https://github.com/moonduming/tokio-shortlink.git
   cd tokio-shortlink
   ```
2. 复制并编辑配置文件  
   ```bash
   cp .env.example .env
   # 编辑 .env，填写您的环境变量
   ```
3. 构建并运行  
   ```bash
   cargo run
   ```

## ⚠️ 限流集成测试须知

本项目的多个集成测试模块（如 `tests/rate_limit.rs`, `tests/register.rs`, `tests/token_auth.rs` 等）涉及 Redis 中的 IP 和用户限流逻辑，状态具有全局性。

**建议所有集成测试文件单独运行**，避免批量执行时相互干扰，导致测试不稳定或误报。

示例：
```bash
cargo test --test rate_limit
cargo test --test register
cargo test --test token_auth
```

如需批量执行，请确保提前清理 Redis 状态并合理安排测试顺序。
