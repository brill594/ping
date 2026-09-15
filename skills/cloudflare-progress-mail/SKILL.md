---
name: cloudflare-progress-mail
description: Send concise email checkpoints for long-running local agent tasks through the preconfigured ping-agent-mail service. Use when work will continue while the user may be away, or when the user asks for progress or completion notifications by email. Do not use for short interactive work or as a substitute for normal in-session updates.
---

# Cloudflare Progress Mail

Use `ping-agent-mail`; never access or request its Cloudflare token, sender, recipient, or account ID. Those are service-owned policy.

For work likely to take more than 15 minutes:

1. Run `ping-agent-mail health` against the local service on port 9109. If unavailable, continue the requested work and tell the user once in the normal conversation; do not attempt to configure credentials.
2. Send `started` after the task scope and success criteria are known.
3. Send `progress` only at meaningful checkpoints or roughly every 15 minutes. The service may suppress updates according to its configured interval.
4. Send exactly one `completed` when all promised verification succeeds, or one `failed` when the task ends without success. Do not label incomplete or partially verified work as completed.

Use deterministic command arguments, not generated routing logic:

```sh
ping-agent-mail notify \
  --task-id '<stable short id>' \
  --status started|progress|completed|failed \
  --summary '<one-line outcome or current state>' \
  --details '<verified work, current blocker, and next step>' \
  --progress 0..100
```

Keep summaries under 512 bytes and details under 32 KiB. Omit `--progress` when a percentage would be invented. Do not include secrets, authentication material, private keys, full proxy URLs, or unnecessarily sensitive logs in notification text.

Email is an out-of-band convenience, not proof that work succeeded. Continue to report the final result in the user conversation.
