//! A local HTTP server for runtime tests. It answers on 127.0.0.1 only, so
//! automated checks never reach a live model service or the npm registry.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// One request the server received.
#[derive(Debug, Clone)]
pub struct Received {
    pub method: String,
    pub path: String,
    /// Header names in lowercase.
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl Received {
    pub fn body_text(&self) -> String {
        String::from_utf8(self.body.clone()).unwrap()
    }
}

/// A canned reply.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub reason: &'static str,
    pub body: String,
}

impl Reply {
    pub fn json(body: impl Into<String>) -> Reply {
        Reply { status: 200, reason: "OK", body: body.into() }
    }

    pub fn status(status: u16, reason: &'static str, body: impl Into<String>) -> Reply {
        Reply { status, reason, body: body.into() }
    }
}

/// A server that answers the given replies in order, one per connection.
pub struct Server {
    pub base_url: String,
    received: Arc<Mutex<Vec<Received>>>,
    handle: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(replies: Vec<Reply>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        let handle = std::thread::spawn(move || {
            for reply in replies {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream);
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("").to_string();
                let mut headers = BTreeMap::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let line = line.trim_end();
                    if line.is_empty() {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        headers.insert(name.trim().to_lowercase(), value.trim().to_string());
                    }
                }
                let length: usize = headers.get("content-length").and_then(|value| value.parse().ok()).unwrap_or(0);
                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).unwrap();
                log.lock().unwrap().push(Received { method, path, headers, body });
                let mut stream = reader.into_inner();
                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    reply.status,
                    reply.reason,
                    reply.body.len(),
                    reply.body
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.flush().unwrap();
            }
        });
        Server { base_url: format!("http://127.0.0.1:{port}/v1"), received, handle: Some(handle) }
    }

    /// The root without the `/v1` suffix.
    pub fn origin(&self) -> String {
        self.base_url.trim_end_matches("/v1").to_string()
    }

    /// Wait for every reply to be sent, then return what was received.
    pub fn finish(mut self) -> Vec<Received> {
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
        self.received.lock().unwrap().clone()
    }
}

/// A port nothing listens on.
pub fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}/v1")
}
