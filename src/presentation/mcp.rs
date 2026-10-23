//! MCP (Model Context Protocol) stdio server.
//!
//! All logging goes to stderr; stdout carries only JSON-RPC frames.
//! Implements: initialize, resources/list, resources/read, tools/list, tools/call.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::application::{ExtractApk, ListApps};
use crate::domain::AdbCommandExecutor;

#[derive(Debug, Deserialize)]
struct Request {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

impl Response {
    fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }
    fn err(id: Value, code: i64, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self { jsonrpc: "2.0", id, result: None, error: Some(json!({ "code": code, "message": msg })) }
    }
}

pub fn run(executor: &dyn AdbCommandExecutor) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };

        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => { log::error!("JSON-RPC parse error: {e}"); continue; }
        };

        let id = req.id.clone().unwrap_or(Value::Null);
        let resp = dispatch(&req, executor, id);
        let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap_or_default());
        let _ = out.flush();
    }
}

fn dispatch(req: &Request, executor: &dyn AdbCommandExecutor, id: Value) -> Response {
    match req.method.as_str() {
        "initialize" => Response::ok(id, json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "resources": { "subscribe": false }, "tools": {} },
            "serverInfo": { "name": "guitrator-apk", "version": env!("CARGO_PKG_VERSION") }
        })),

        "initialized" => Response::ok(id, Value::Null),

        "resources/list" => Response::ok(id, json!({
            "resources": [{
                "uri": "adb://device/installed_apps",
                "name": "Installed Apps",
                "mimeType": "text/plain"
            }]
        })),

        "resources/read" => match ListApps::new(executor).execute() {
            Ok(apps) => {
                let text = apps.iter()
                    .map(|a| format!("{} → {}", a.package_name, a.apk_path))
                    .collect::<Vec<_>>()
                    .join("\n");
                Response::ok(id, json!({
                    "contents": [{ "uri": "adb://device/installed_apps", "mimeType": "text/plain", "text": text }]
                }))
            }
            Err(e) => Response::err(id, -32000, e.to_string()),
        },

        "tools/list" => Response::ok(id, json!({
            "tools": [{
                "name": "extract_apk",
                "description": "Pull an APK from the connected Android device to the local filesystem.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "package_name": { "type": "string", "description": "Android package name, e.g. com.example.app" },
                        "apk_path":     { "type": "string", "description": "Device-side APK path from pm list packages -f" },
                        "destination":  { "type": "string", "description": "Local directory to save the APK (default: .)" }
                    },
                    "required": ["package_name", "apk_path"]
                }
            }]
        })),

        "tools/call" => {
            let args = req.params.as_ref()
                .and_then(|p| p.get("arguments"))
                .and_then(|a| a.as_object());

            let pkg  = args.and_then(|a| a.get("package_name")).and_then(Value::as_str).map(str::to_string);
            let path = args.and_then(|a| a.get("apk_path")).and_then(Value::as_str).map(str::to_string);
            let dest = args.and_then(|a| a.get("destination")).and_then(Value::as_str).unwrap_or(".").to_string();

            match (pkg, path) {
                (Some(pkg), Some(path)) => match ExtractApk::new(executor).execute(&path, Path::new(&dest), &pkg) {
                    Ok(saved) => Response::ok(id, json!({
                        "content": [{ "type": "text", "text": format!("APK saved to {}", saved.display()) }]
                    })),
                    Err(e) => Response::err(id, -32000, e.to_string()),
                },
                _ => Response::err(id, -32602, "missing required arguments: package_name, apk_path"),
            }
        }

        method => Response::err(id, -32601, format!("method not found: {method}")),
    }
}
