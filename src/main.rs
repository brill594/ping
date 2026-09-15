mod cloudflare;
mod config;
mod protocol;
mod service;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use protocol::{Notification, Status};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };

    match command {
        "serve" => {
            let config_path = single_option(&args[1..], "--config")?
                .map(PathBuf::from)
                .unwrap_or_else(config::default_config_path);
            service::serve(&config_path)
        }
        "notify" => notify(&args[1..]),
        "health" => {
            reject_args(&args[1..])?;
            let response = service::request("/healthz", &protocol::Request::Health)?;
            print_response(response)
        }
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn notify(args: &[String]) -> Result<(), String> {
    let task_id = required_option(args, "--task-id")?;
    let status = required_option(args, "--status")?.parse::<Status>()?;
    let summary = required_option(args, "--summary")?;
    let progress = optional_u8(args, "--progress")?;
    let details = single_option(args, "--details")?;
    reject_unknown_options(
        args,
        &[
            "--task-id",
            "--status",
            "--summary",
            "--progress",
            "--details",
        ],
    )?;

    let notification = Notification {
        task_id,
        status,
        summary,
        details,
        progress,
    };
    notification.validate()?;
    let response = service::request("/v1/notify", &protocol::Request::Notify(notification))?;
    print_response(response)
}

fn print_response(response: protocol::Response) -> Result<(), String> {
    if response.ok {
        println!("{}", response.message);
        Ok(())
    } else {
        Err(response.message)
    }
}

fn required_option(args: &[String], name: &str) -> Result<String, String> {
    single_option(args, name)?.ok_or_else(|| format!("missing required option {name}"))
}

fn single_option(args: &[String], name: &str) -> Result<Option<String>, String> {
    let mut found = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == name {
            if found.is_some() {
                return Err(format!("option {name} may only be used once"));
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("option {name} requires a value"))?;
            if value.starts_with("--") {
                return Err(format!("option {name} requires a value"));
            }
            found = Some(value.clone());
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(found)
}

fn optional_u8(args: &[String], name: &str) -> Result<Option<u8>, String> {
    single_option(args, name)?
        .map(|value| {
            value
                .parse::<u8>()
                .map_err(|_| format!("{name} must be an integer from 0 to 100"))
                .and_then(|value| {
                    if value <= 100 {
                        Ok(value)
                    } else {
                        Err(format!("{name} must be an integer from 0 to 100"))
                    }
                })
        })
        .transpose()
}

fn reject_unknown_options(args: &[String], known: &[&str]) -> Result<(), String> {
    let mut index = 0;
    while index < args.len() {
        if !known.contains(&args[index].as_str()) {
            return Err(format!("unknown option {}", args[index]));
        }
        index += 2;
    }
    Ok(())
}

fn reject_args(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(format!("unexpected argument {}", args[0]))
    }
}

fn usage() -> String {
    "Usage:\n  ping-agent-mail serve [--config PATH]\n  ping-agent-mail health\n  ping-agent-mail notify --task-id ID --status started|progress|completed|failed --summary TEXT [--progress 0..100] [--details TEXT]".into()
}
