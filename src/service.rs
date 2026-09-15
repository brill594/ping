use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use tiny_http::{
    Header, Method, Request as HttpRequest, Response as HttpResponse, Server, StatusCode,
};

use crate::cloudflare;
use crate::config::Config;
use crate::protocol::{MAX_REQUEST_BYTES, Request, Response, Status};

const CLIENT_BASE_URL: &str = "http://127.0.0.1:9109";

pub fn serve(config_path: &Path) -> Result<(), String> {
    let config = Config::load(config_path)?;
    let server = Server::http(&config.listen)
        .map_err(|error| format!("cannot listen on {}: {error}", config.listen))?;
    eprintln!("listening on http://{}", config.listen);

    let mut last_progress = HashMap::new();
    for request in server.incoming_requests() {
        handle_http(request, &config, &mut last_progress);
    }
    Ok(())
}

pub fn request(path: &str, request: &Request) -> Result<Response, String> {
    let url = format!("{CLIENT_BASE_URL}{path}");
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
        .new_agent();
    let mut response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .send_json(request)
        .map_err(|error| format!("cannot reach local service at {CLIENT_BASE_URL}: {error}"))?;
    response
        .body_mut()
        .read_json::<Response>()
        .map_err(|error| format!("invalid local service response: {error}"))
}

fn handle_http(
    mut request: HttpRequest,
    config: &Config,
    last_progress: &mut HashMap<String, Instant>,
) {
    let response = match (request.method(), request.url()) {
        (&Method::Get, "/healthz") => Response::ok("service is ready"),
        (&Method::Post, "/healthz") => read_request(&mut request)
            .and_then(|parsed| match parsed {
                Request::Health => Ok(Response::ok("service is ready")),
                Request::Notify(_) => Err("health endpoint only accepts health requests".into()),
            })
            .unwrap_or_else(Response::error),
        (&Method::Post, "/v1/notify") => read_request(&mut request)
            .and_then(|parsed| match parsed {
                Request::Notify(notification) => {
                    notification.validate()?;
                    if matches!(notification.status, Status::Progress)
                        && progress_is_too_soon(
                            last_progress.get(&notification.task_id),
                            config.min_progress_interval_secs,
                        )
                    {
                        return Ok(Response::ok(
                            "progress notification suppressed by service interval",
                        ));
                    }
                    cloudflare::send(config, &notification)?;
                    match notification.status {
                        Status::Progress => {
                            last_progress.insert(notification.task_id, Instant::now());
                        }
                        Status::Completed | Status::Failed => {
                            last_progress.remove(&notification.task_id);
                        }
                        Status::Started => {}
                    }
                    Ok(Response::ok("notification sent"))
                }
                Request::Health => Err("notify endpoint only accepts notify requests".into()),
            })
            .unwrap_or_else(Response::error),
        _ => {
            respond_json(request, StatusCode(404), Response::error("not found"));
            return;
        }
    };

    let status = if response.ok {
        StatusCode(200)
    } else {
        StatusCode(400)
    };
    respond_json(request, status, response);
}

fn read_request(request: &mut HttpRequest) -> Result<Request, String> {
    let length = request
        .body_length()
        .ok_or_else(|| "Content-Length is required".to_string())?;
    if length > MAX_REQUEST_BYTES {
        return Err(format!("request exceeds {MAX_REQUEST_BYTES} bytes"));
    }
    let content_type_is_json = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Content-Type"))
        .is_some_and(|header| header.value.as_str().split(';').next() == Some("application/json"));
    if !content_type_is_json {
        return Err("Content-Type must be application/json".into());
    }

    let mut body = Vec::with_capacity(length);
    request
        .as_reader()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| format!("cannot read request: {error}"))?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(format!("request exceeds {MAX_REQUEST_BYTES} bytes"));
    }
    serde_json::from_slice(&body).map_err(|error| format!("invalid request: {error}"))
}

fn respond_json(request: HttpRequest, status: StatusCode, response: Response) {
    let body = serde_json::to_vec(&response)
        .unwrap_or_else(|_| br#"{"ok":false,"message":"cannot encode response"}"#.to_vec());
    let content_type =
        Header::from_bytes("Content-Type", "application/json").expect("static header is valid");
    if let Err(error) = request.respond(
        HttpResponse::from_data(body)
            .with_status_code(status)
            .with_header(content_type),
    ) {
        eprintln!("cannot write local HTTP response: {error}");
    }
}

fn progress_is_too_soon(previous: Option<&Instant>, interval_secs: u64) -> bool {
    previous.is_some_and(|instant| instant.elapsed() < Duration::from_secs(interval_secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_existing_progress_timestamps_are_throttled() {
        let now = Instant::now();
        assert!(progress_is_too_soon(Some(&now), 60));
        assert!(!progress_is_too_soon(None, 60));
        assert!(!progress_is_too_soon(Some(&now), 0));
    }
}
