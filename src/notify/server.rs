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

#[derive(Deserialize, Debug, Default)]
struct MagnetPayload {
    magnet: Option<String>,
    url: Option<String>,
    target: Option<String>,
    text: Option<String>,
}

pub fn spawn_background_server(config: TelegramConfig, port: u16) {
    std::thread::spawn(move || {
        if let Err(e) = run_server(config, port) {
            eprintln!("  ⚠️ Notification HTTP server stopped: {e}");
        }
    });
}

pub fn run_server(config: TelegramConfig, port: u16) -> Result<()> {
    let addr = format!("0.0.0.0:{port}");
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

    if method == "OPTIONS" {
        return write_cors_preflight(&mut stream);
    }

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

    if method == "POST" && path.starts_with("/seedr-webhook") {
        return handle_seedr_webhook(&mut stream, &mut reader, content_length, path, config);
    }

    if method == "POST" && (path == "/magnet" || path == "/add-magnet" || path == "/torrent") {
        return handle_magnet_post(&mut stream, &mut reader, content_length, &content_type, config);
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


fn write_cors_preflight(stream: &mut TcpStream) -> Result<()> {
    let response = "HTTP/1.1 204 No Content\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
        Content-Length: 0\r\n\
        Connection: close\r\n\r\n";
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
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
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\r\n\
        {json_body}\n",
        json_body.len() + 1
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[derive(Deserialize, Debug)]
#[serde(tag = "event", rename_all = "snake_case")]
enum SeedrEvent {
    Completed {
        file_name: String,
        destination_path: Option<String>,
        total_bytes: u64,
    },
    Failed {
        error: String,
    },
    #[serde(other)]
    Ignored,
}

fn handle_seedr_webhook(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    content_length: usize,
    path: &str,
    config: &TelegramConfig,
) -> Result<()> {
    let mut body_bytes = vec![0u8; content_length.min(65536)];
    reader.read_exact(&mut body_bytes)?;
    let body = String::from_utf8_lossy(&body_bytes);

    let hash = path.split('?').nth(1).and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut kv = pair.split('=');
            if kv.next()? == "hash" {
                kv.next()
            } else {
                None
            }
        })
    });

    if let Ok(event) = serde_json::from_str::<SeedrEvent>(&body) {
        match event {
            SeedrEvent::Completed {
                file_name,
                destination_path,
                total_bytes,
                ..
            } => {
                let _ = crate::modules::torrent::seedr::handle_seedr_completion(
                    hash,
                    &file_name,
                    total_bytes,
                    destination_path.as_deref(),
                    config,
                );
            }
            SeedrEvent::Failed { error, .. } => {
                let _ = crate::modules::torrent::seedr::handle_seedr_failure(hash, &error, config);
            }
            SeedrEvent::Ignored => {}
        }
    }

    write_response(stream, 200, r#"{"status":"received"}"#)
}

fn extract_magnet(body: &str, content_type: &str) -> String {
    if content_type.contains("application/json") {
        if let Ok(parsed) = serde_json::from_str::<MagnetPayload>(body) {
            if let Some(m) = parsed.magnet.or(parsed.url).or(parsed.target).or(parsed.text) {
                if !m.trim().is_empty() {
                    return m.trim().to_string();
                }
            }
        }
    }

    // form-encoded: magnet=... or url=... or target=...
    for key in ["magnet", "url", "target", "text"] {
        let prefix = format!("{key}=");
        if let Some(rest) = body.strip_prefix(&prefix) {
            let val = rest.split('&').next().unwrap_or(rest);
            if !val.trim().is_empty() {
                return url_decode(val.trim());
            }
        }
        // search anywhere in form body
        for pair in body.split('&') {
            let mut kv = pair.splitn(2, '=');
            if kv.next() == Some(key) {
                if let Some(v) = kv.next() {
                    if !v.trim().is_empty() {
                        return url_decode(v.trim());
                    }
                }
            }
        }
    }

    body.trim().to_string()
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            if let (Some(c1), Some(c2)) = (bytes.next(), bytes.next()) {
                if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&[c1, c2]).unwrap_or(""), 16) {
                    out.push(v as char);
                }
            }
        } else if b == b'+' {
            out.push(' ');
        } else {
            out.push(b as char);
        }
    }
    out
}

fn handle_magnet_post(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    content_length: usize,
    content_type: &str,
    config: &TelegramConfig,
) -> Result<()> {
    let mut body_bytes = vec![0u8; content_length.min(65536)];
    if content_length > 0 {
        reader.read_exact(&mut body_bytes)?;
    }
    let body = String::from_utf8_lossy(&body_bytes);

    let magnet = extract_magnet(&body, content_type);
    if magnet.is_empty() {
        return write_response(
            &mut *stream,
            400,
            r#"{"error":"Missing magnet (use JSON {\"magnet\":\"...\"} or raw body)"}"#,
        );
    }

    match crate::modules::torrent::cli::add_magnet_pipeline(&magnet, config) {
        Ok(result) => {
            let json = serde_json::to_string(&result).unwrap_or_else(|_| {
                r#"{"status":"ok","message":"magnet accepted"}"#.to_string()
            });
            write_response(&mut *stream, 200, &json)
        }
        Err(e) => {
            let err_json = format!(r#"{{"error":"{e}"}}"#);
            write_response(&mut *stream, 400, &err_json)
        }
    }
}
