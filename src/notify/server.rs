use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use super::config::TelegramConfig;
use super::system::send_custom_notification;

#[derive(Deserialize, Debug, Default)]
struct NotifyPayload {
    title: Option<String>,
    message: Option<String>,
    text: Option<String>,
    level: Option<String>,
}

pub fn spawn_background_server(config: TelegramConfig, port: u16) {
    std::thread::spawn(move || {
        if let Err(e) = run_server(config, port) {
            eprintln!("  ⚠️ Notification HTTP server stopped: {e}");
        }
    });
}

pub fn run_server(config: TelegramConfig, port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .with_context(|| format!("Failed to bind notification server to {addr}"))?;

    println!("  ⚡ Notification Webhook Server listening on http://{addr}");
    let cfg = Arc::new(config);

    for stream in listener.incoming().flatten() {
        let cfg_clone = Arc::clone(&cfg);
        std::thread::spawn(move || {
            let _ = handle_client(stream, &cfg_clone);
        });
    }
    Ok(())
}

fn handle_client(mut stream: TcpStream, config: &TelegramConfig) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let (content_length, content_type) = parse_headers(&mut reader)?;

    if method == "GET" && (path == "/health" || path == "/") {
        return write_response(
            &mut stream,
            200,
            r#"{"status":"ok","service":"ryoiki-notify"}"#,
        );
    }

    if method == "POST" && (path == "/notify" || path == "/send") {
        return handle_notify_post(
            &mut stream,
            &mut reader,
            content_length,
            &content_type,
            config,
        );
    }

    write_response(&mut stream, 404, r#"{"error":"Not Found"}"#)
}

fn parse_headers(reader: &mut BufReader<TcpStream>) -> Result<(usize, String)> {
    let mut content_length = 0;
    let mut content_type = String::new();

    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = trimmed.split(':').nth(1) {
                content_length = val.trim().parse().unwrap_or(0);
            }
        } else if lower.starts_with("content-type:") {
            if let Some(val) = trimmed.split(':').nth(1) {
                content_type = val.trim().to_string();
            }
        }
    }
    Ok((content_length, content_type))
}

fn handle_notify_post(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    content_length: usize,
    content_type: &str,
    config: &TelegramConfig,
) -> Result<()> {
    let mut body_bytes = vec![0u8; content_length.min(65536)];
    reader.read_exact(&mut body_bytes)?;
    let body = String::from_utf8_lossy(&body_bytes);

    let (title, message, level) = extract_payload(&body, content_type);
    if message.is_empty() {
        return write_response(
            &mut *stream,
            400,
            r#"{"error":"Missing message or text field"}"#,
        );
    }

    match send_custom_notification(config, &message, title.as_deref(), &level) {
        Ok(()) => write_response(&mut *stream, 200, r#"{"status":"sent"}"#),
        Err(e) => {
            let err_json = format!(r#"{{"error":"Failed to dispatch Telegram alert: {e}"}}"#);
            write_response(&mut *stream, 500, &err_json)
        }
    }
}

fn extract_payload(body: &str, content_type: &str) -> (Option<String>, String, String) {
    if content_type.contains("application/json") {
        if let Ok(parsed) = serde_json::from_str::<NotifyPayload>(body) {
            let msg = parsed.message.or(parsed.text).unwrap_or_default();
            let lvl = parsed.level.unwrap_or_else(|| "info".to_string());
            return (parsed.title, msg, lvl);
        }
    }

    if body.starts_with("text=") || body.starts_with("message=") {
        let text = body.split('=').nth(1).unwrap_or(body);
        return (None, text.to_string(), "info".to_string());
    }

    (None, body.trim().to_string(), "info".to_string())
}

fn write_response(stream: &mut TcpStream, status_code: u16, json_body: &str) -> Result<()> {
    let status_text = match status_code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Internal Server Error",
    };

    let response = format!(
        "HTTP/1.1 {status_code} {status_text}\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\r\n\
        {json_body}\n",
        json_body.len() + 1
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}
