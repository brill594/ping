# ping-agent-mail

[English](README.md)

一个低资源占用的本地 Rust 服务，用于让长时间运行的 Agent 通过 [Cloudflare Email Sending HTTPS API](https://developers.cloudflare.com/email-service/api/send-emails/rest-api/) 发送进度通知。

Cloudflare Account ID、API Token、发件地址和收件地址由服务端配置统一管理。本地 Agent 只能通过 HTTP API 提交任务 ID、状态和进度内容，不能选择邮件路由，也无法读取凭据。

## 构建

```sh
cargo build --release
```

构建产物位于 `target/release/ping-agent-mail`。

## 配置与运行

使用前需要为 Cloudflare 账户启用 Email Sending，完成发件域配置，验证目标收件地址，并创建具有邮件发送权限的 API Token。

```sh
mkdir -p ~/.config/ping-agent-mail
cp config.example.toml ~/.config/ping-agent-mail/config.toml
chmod 600 ~/.config/ping-agent-mail/config.toml
$EDITOR ~/.config/ping-agent-mail/config.toml
target/release/ping-agent-mail serve
```

配置文件示例：

```toml
account_id = "your-cloudflare-account-id"
api_token = "your-cloudflare-api-token"
from = "agent@example.com"
to = "you@example.net"

listen = "127.0.0.1:9109"
min_progress_interval_secs = 900
subject_prefix = "[Agent]"
```

服务默认监听 `127.0.0.1:9109`，以前台方式运行，只记录运行错误，不记录 Token 或邮件正文。建议使用系统的用户级服务管理器保持服务运行。

macOS 用户可以参考 `packaging/moe.brill.ping-agent-mail.plist` 配置 LaunchAgent。在其他用户目录中安装时，需要相应修改 plist 内的绝对路径。

## 发送任务通知

检查服务状态：

```sh
target/release/ping-agent-mail health
```

发送进度通知：

```sh
target/release/ping-agent-mail notify \
  --task-id build-42 \
  --status progress \
  --progress 60 \
  --summary "发布版本仍在构建" \
  --details "编译已经完成，接下来运行集成测试。"
```

支持以下任务状态：

- `started`：任务开始
- `progress`：任务进行中
- `completed`：任务成功完成
- `failed`：任务失败或无法继续

服务会根据 `min_progress_interval_secs` 抑制过于频繁的 `progress` 邮件。`started`、`completed` 和 `failed` 通知不会受到该间隔限制。

## HTTP API

Agent 也可以直接请求 `POST http://127.0.0.1:9109/v1/notify`，并将 `Content-Type` 设置为 `application/json`：

```json
{
  "type": "notify",
  "task_id": "build-42",
  "status": "progress",
  "summary": "发布版本仍在构建",
  "details": "编译已经完成，接下来运行集成测试。",
  "progress": 60
}
```

健康检查地址为 `GET /healthz`。服务会拒绝未知 JSON 字段，因此调用方无法注入 `to`、`from`、凭据、邮件头或附件。

## 安装 Skill

将 `skills/cloudflare-progress-mail` 复制或链接到 Agent 的 skill 目录，并确保：

- `ping-agent-mail` 已加入 `PATH`
- 本地服务正在运行
- 当前 Agent 会话已重新启动并加载新 skill

该 skill 会指导 Agent 在长任务开始、重要检查点、成功完成或失败时发送简洁通知，同时避免在邮件中泄露凭据或无关的敏感日志。

## 安全边界

- Unix 系统上，如果配置文件允许组用户或其他用户读取，服务会拒绝启动。
- 默认仅监听本机回环地址的 9109 端口。绑定到其他网络接口时必须额外部署访问控制。
- 本地通知协议不接受发件地址、收件地址或认证信息。
- 单个请求最大为 64 KiB，用户内容在生成 HTML 邮件时会进行转义。
- 出站 HTTPS 使用 rustls 验证服务端证书。
- 服务不会自动重试邮件 POST 请求，避免在网络结果不明确时产生重复邮件。

## 许可证

本项目采用 MIT License，详见 [LICENSE](LICENSE)。
