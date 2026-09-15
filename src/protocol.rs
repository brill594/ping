use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Health,
    Notify(Notification),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub task_id: String,
    pub status: Status,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
}

impl Notification {
    pub fn validate(&self) -> Result<(), String> {
        if self.task_id.trim().is_empty() || self.task_id.len() > 128 {
            return Err("task_id must contain 1 to 128 bytes".into());
        }
        if self.task_id.contains(['\r', '\n']) {
            return Err("task_id must not contain a newline".into());
        }
        if self.summary.trim().is_empty() || self.summary.len() > 512 {
            return Err("summary must contain 1 to 512 bytes".into());
        }
        if self.summary.contains(['\r', '\n']) {
            return Err("summary must not contain a newline".into());
        }
        if self
            .details
            .as_ref()
            .is_some_and(|value| value.len() > 32 * 1024)
        {
            return Err("details must not exceed 32768 bytes".into());
        }
        if self.progress.is_some_and(|value| value > 100) {
            return Err("progress must be from 0 to 100".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Started,
    Progress,
    Completed,
    Failed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Progress => "progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl FromStr for Status {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "started" => Ok(Self::Started),
            "progress" => Ok(Self::Progress),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err("status must be started, progress, completed, or failed".into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    pub ok: bool,
    pub message: String,
}

impl Response {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_limits_keep_local_requests_bounded() {
        let notification = Notification {
            task_id: "build-42".into(),
            status: Status::Progress,
            summary: "still compiling".into(),
            details: None,
            progress: Some(101),
        };
        assert!(notification.validate().is_err());
    }

    #[test]
    fn agent_cannot_add_mail_routing_fields() {
        let request = br#"{
            "type":"notify",
            "task_id":"build-42",
            "status":"progress",
            "summary":"working",
            "to":"attacker@example.com"
        }"#;
        assert!(serde_json::from_slice::<Request>(request).is_err());
    }
}
