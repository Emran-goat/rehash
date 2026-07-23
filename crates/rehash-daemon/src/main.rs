use std::path::{Path, PathBuf};
use std::fs;

use anyhow::Result;
use notify::{Config, EventKind, RecursiveMode, RecommendedWatcher, Watcher};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

#[cfg(unix)]
use tokio::net::{UnixListener as IpcListener, UnixStream as IpcStream};
#[cfg(windows)]
use tokio::net::{TcpListener as IpcListener, TcpStream as IpcStream};

const SOURCE_FILES: &[&str] = &[
    "Cargo.toml", "Makefile", "CMakeLists.txt", "package.json",
];
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "c", "h", "cpp", "js", "ts", "py", "toml", "json", "cmake", "mk",
];
const IGNORE_DIRS: &[&str] = &["target", "node_modules", "build", ".git"];

struct WatchEvent {
    path: PathBuf,
    kind: EventKind,
}

#[derive(Deserialize)]
struct IpcRequest {
    #[serde(rename = "type")]
    kind: String,
    path: Option<String>,
}

#[derive(Serialize)]
struct IpcResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

fn should_ignore(path: &Path) -> bool {
    path.components().any(|c| {
        if let std::path::Component::Normal(s) = c {
            if let Some(s) = s.to_str() {
                return IGNORE_DIRS.contains(&s);
            }
        }
        false
    })
}

fn is_source_file(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
        if SOURCE_FILES.contains(&name) {
            return true;
        }
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SOURCE_EXTENSIONS.contains(&e))
        .unwrap_or(false)
}

fn get_cache_stats(dir: &Path) -> (u64, usize) {
    let mut size = 0;
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let (s, c) = get_cache_stats(&path);
                size += s;
                count += c;
            } else {
                size += entry.metadata().map(|m| m.len()).unwrap_or(0);
                count += 1;
            }
        }
    }
    (size, count)
}

async fn handle_connection(stream: IpcStream, cache_dir: PathBuf) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let response = match serde_json::from_str::<IpcRequest>(line) {
                    Ok(req) => match req.kind.as_str() {
                        "ping" => IpcResponse {
                            status: "ok".into(),
                            data: Some(serde_json::json!("pong")),
                        },
                        "stats" => {
                            let (size, count) = get_cache_stats(&cache_dir);
                            IpcResponse {
                                status: "ok".into(),
                                data: Some(serde_json::json!({
                                    "size": size,
                                    "entries": count,
                                })),
                            }
                        }
                        "invalidate" => {
                            tracing::info!("invalidate: {:?}", req.path);
                            IpcResponse {
                                status: "ok".into(),
                                data: None,
                            }
                        }
                        _ => IpcResponse {
                            status: "error".into(),
                            data: Some(serde_json::json!("unknown command")),
                        },
                    },
                    Err(_) => IpcResponse {
                        status: "error".into(),
                        data: Some(serde_json::json!("parse error")),
                    },
                };

                let json = serde_json::to_string(&response).unwrap_or_default();
                let _ = writer.write_all(format!("{}\n", json).as_bytes()).await;
                let _ = writer.flush().await;
            }
        }
    }
}

fn start_watcher(tx: mpsc::Sender<WatchEvent>) {
    std::thread::spawn(move || {
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(notify_tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("failed to create watcher: {:?}", e);
                return;
            }
        };
        let cwd = match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("no cwd: {:?}", e);
                return;
            }
        };
        if let Err(e) = watcher.watch(&cwd, RecursiveMode::Recursive) {
            tracing::error!("failed to start watching: {:?}", e);
            return;
        }

        for event in notify_rx {
            match event {
                Ok(event) => {
                    for path in event.paths {
                        if should_ignore(&path) || !is_source_file(&path) {
                            continue;
                        }
                        let _ = tx.blocking_send(WatchEvent {
                            path,
                            kind: event.kind,
                        });
                    }
                }
                Err(e) => tracing::error!("watch error: {:?}", e),
            }
        }
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let cache_dir = PathBuf::from(home).join(".cache").join("rehash");
    fs::create_dir_all(&cache_dir)?;

    let (watch_tx, mut watch_rx) = mpsc::channel::<WatchEvent>(256);

    start_watcher(watch_tx);

    tokio::spawn(async move {
        while let Some(event) = watch_rx.recv().await {
            tracing::info!("{:?}: {}", event.kind, event.path.display());
        }
    });

    #[cfg(unix)]
    let listener = {
        let sock_path = cache_dir.join("rehash.sock");
        let _ = fs::remove_file(&sock_path);
        IpcListener::bind(&sock_path)?
    };

    #[cfg(windows)]
    let listener = IpcListener::bind("127.0.0.1:47831").await?;

    tracing::info!("rehash daemon ready");

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let cache_dir = cache_dir.clone();
                        tokio::spawn(handle_connection(stream, cache_dir));
                    }
                    Err(e) => tracing::error!("accept error: {:?}", e),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutting down");
                break;
            }
        }
    }

    Ok(())
}
