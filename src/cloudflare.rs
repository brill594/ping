use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use crate::config::Config;
use crate::protocol::Notification;

const API_ROOT: &str = "https://api.cloudflare.com/client/v4";

#[derive(Debug, Deserialize)]
struct ApiEnvelope {
    success: bool,
    #[serde(default)]
    errors: Vec<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: Option<i64>,
    message: Option<String>,
}

pub fn send(config: &Config, notification: &Notification) -> Result<(), String> {
    let (url, payload) = request_parts(config, notification);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .https_only(true)
        .max_redirects(0)
        .build()
        .new_agent();

    let mut response = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", config.api_token))
        .header("Content-Type", "application/json")
        .send_json(&payload)
        .map_err(format_http_error)?;

    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("cannot read Cloudflare response: {error}"))?;
    let envelope: ApiEnvelope = serde_json::from_str(&body).map_err(|error| {
        if status >= 400 {
            format!("Cloudflare returned HTTP {status}")
        } else {
            format!("invalid Cloudflare response: {error}")
        }
    })?;
    if envelope.success {
        return Ok(());
    }

    let details = envelope
        .errors
        .iter()
        .map(|error| match (&error.code, &error.message) {
            (Some(code), Some(message)) => format!("{code}: {message}"),
            (_, Some(message)) => message.clone(),
            _ => "unspecified API error".into(),
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(if details.is_empty() {
        format!("Cloudflare rejected the email with HTTP {status}")
    } else {
        format!("Cloudflare rejected the email: {details}")
    })
}

fn request_parts(config: &Config, notification: &Notification) -> (String, serde_json::Value) {
    let url = format!(
        "{API_ROOT}/accounts/{}/email/sending/send",
        config.account_id
    );
    let subject = format!(
        "{} [{}] {}: {}",
        config.subject_prefix,
        notification.status.as_str(),
        notification.task_id,
        notification.summary
    );
    let text = text_body(notification);
    let html = html_body(notification);
    let payload = json!({
        "to": config.to,
        "from": config.from,
        "subject": subject,
        "text": text,
        "html": html,
    });
    (url, payload)
}

fn format_http_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(code) => format!("Cloudflare returned HTTP {code}"),
        other => format!("Cloudflare request failed: {other}"),
    }
}

fn text_body(notification: &Notification) -> String {
    let mut body = format!(
        "Task: {}\nStatus: {}\nSummary: {}",
        notification.task_id,
        notification.status.as_str(),
        notification.summary
    );
    if let Some(progress) = notification.progress {
        body.push_str(&format!("\nProgress: {progress}%"));
    }
    if let Some(details) = &notification.details {
        body.push_str("\n\nDetails:\n");
        body.push_str(details);
    }
    body
}

fn html_body(notification: &Notification) -> String {
    let progress = notification
        .progress
        .map(|value| format!("<p><strong>Progress:</strong> {value}%</p>"))
        .unwrap_or_default();
    let details = notification
        .details
        .as_ref()
        .map(|value| format!("<h2>Details</h2><pre>{}</pre>", escape_html(value)))
        .unwrap_or_default();
    format!(
        "<h1>Agent task update</h1><p><strong>Task:</strong> {}</p><p><strong>Status:</strong> {}</p><p><strong>Summary:</strong> {}</p>{progress}{details}",
        escape_html(&notification.task_id),
        notification.status.as_str(),
        escape_html(&notification.summary),
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Status;

    fn config() -> Config {
        Config {
            account_id: "account-from-service".into(),
            api_token: "secret-token".into(),
            from: "agent@example.com".into(),
            to: "owner@example.net".into(),
            listen: "127.0.0.1:9109".into(),
            min_progress_interval_secs: 900,
            subject_prefix: "[Agent]".into(),
        }
    }

    #[test]
    fn user_content_is_escaped_in_html_mail() {
        let notification = Notification {
            task_id: "task<&>".into(),
            status: Status::Progress,
            summary: "<script>alert('x')</script>".into(),
            details: Some("a & b".into()),
            progress: Some(50),
        };
        let html = html_body(&notification);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("a &amp; b"));
    }

    #[test]
    fn cloudflare_routing_comes_only_from_service_config() {
        let notification = Notification {
            task_id: "task-1".into(),
            status: Status::Started,
            summary: "starting".into(),
            details: None,
            progress: None,
        };
        let (url, payload) = request_parts(&config(), &notification);
        assert_eq!(
            url,
            "https://api.cloudflare.com/client/v4/accounts/account-from-service/email/sending/send"
        );
        assert_eq!(payload["from"], "agent@example.com");
        assert_eq!(payload["to"], "owner@example.net");
        assert!(payload.get("api_token").is_none());
    }
}
