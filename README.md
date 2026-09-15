# ping-agent-mail

A small local Rust service that lets long-running agents send progress reports through the [Cloudflare Email Sending HTTPS API](https://developers.cloudflare.com/email-service/api/send-emails/rest-api/).

The service owns the Cloudflare account ID, API token, sender, and recipient. Local agents only submit a task ID, status, and report text to its local HTTP API; they cannot choose mail routing or read the credential.

## Build

```sh
cargo build --release
```

The release binary is `target/release/ping-agent-mail`.

## Configure and run

Cloudflare Email Sending must be enabled for the account. The sender domain must be available for sending, the destination must be verified, and the API token must have email-send permission.

```sh
mkdir -p ~/.config/ping-agent-mail
cp config.example.toml ~/.config/ping-agent-mail/config.toml
chmod 600 ~/.config/ping-agent-mail/config.toml
$EDITOR ~/.config/ping-agent-mail/config.toml
target/release/ping-agent-mail serve
```

Run the service under your preferred user service manager for continuous availability. It listens on `127.0.0.1:9109` by default, stays in the foreground, and logs only operational errors; it never logs the token or message body.

On macOS, `packaging/moe.brill.ping-agent-mail.plist` is a user LaunchAgent example. Adjust its absolute paths when installing under a different home directory.

Check readiness and send updates from another shell:

```sh
target/release/ping-agent-mail health
target/release/ping-agent-mail notify \
  --task-id build-42 \
  --status progress \
  --progress 60 \
  --summary "Release build is still running" \
  --details "Compilation finished; integration tests are next."
```

Statuses are `started`, `progress`, `completed`, and `failed`. The service suppresses overly frequent `progress` updates according to its own configuration. Terminal updates are never interval-suppressed.

The same local API is available directly at `POST http://127.0.0.1:9109/v1/notify` with `Content-Type: application/json`:

```json
{
  "type": "notify",
  "task_id": "build-42",
  "status": "progress",
  "summary": "Release build is still running",
  "details": "Compilation finished; integration tests are next.",
  "progress": 60
}
```

`GET /healthz` returns readiness. Unknown JSON fields are rejected, so callers cannot inject `to`, `from`, credentials, headers, or attachments.

## Install the skill

Copy or symlink `skills/cloudflare-progress-mail` into your Codex skills directory, then make sure `ping-agent-mail` is on `PATH` and the service is running. The skill teaches an agent when and how to report long-running work without exposing mail credentials.

## Security boundary

- Configuration files with group/other permissions are rejected on Unix.
- The default listener is loopback-only on port 9109. Binding to other interfaces requires separate access controls.
- Mail identity and destination are not accepted in the local request protocol.
- Requests are capped at 64 KiB and user-provided HTML is escaped.
- HTTPS verification is provided by rustls. The service does not automatically retry POST requests, avoiding duplicate mail after ambiguous network failures.

## License

MIT. See [LICENSE](LICENSE).
