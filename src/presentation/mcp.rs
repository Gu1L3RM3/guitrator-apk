//! MCP (Model Context Protocol) stdio server.
//!
//! All logging goes to stderr; stdout carries only JSON-RPC frames.
//! Implements: initialize, resources/list, resources/read, tools/list, tools/call.
//! Tools: list_apps, device_info, app_details, extract_apk, download_apk.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::application::{ExtractApk, ListApps};
use crate::domain::{AdbCommandExecutor, AdbError};
use crate::infrastructure::download_apk;
use crate::presentation::report;

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
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }
    fn err(id: Value, code: i64, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(json!({ "code": code, "message": msg })),
        }
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
            Err(e) => {
                log::error!("JSON-RPC parse error: {e}");
                continue;
            }
        };

        let id = req.id.clone().unwrap_or(Value::Null);
        let resp = dispatch(&req, executor, id);
        let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap_or_default());
        let _ = out.flush();
    }
}

fn dispatch(req: &Request, executor: &dyn AdbCommandExecutor, id: Value) -> Response {
    match req.method.as_str() {
        "initialize" => Response::ok(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "resources": { "subscribe": false }, "tools": {} },
                "serverInfo": { "name": "guitrator-apk", "version": env!("CARGO_PKG_VERSION") }
            }),
        ),

        "initialized" => Response::ok(id, Value::Null),

        "resources/list" => Response::ok(
            id,
            json!({
                "resources": [{
                    "uri": "adb://device/installed_apps",
                    "name": "Installed Apps",
                    "mimeType": "text/plain"
                }]
            }),
        ),

        "resources/read" => match ListApps::new(executor).execute() {
            Ok(apps) => {
                let text = apps
                    .iter()
                    .map(|a| format!("{} → {}", a.package_name, a.apk_path))
                    .collect::<Vec<_>>()
                    .join("\n");
                Response::ok(
                    id,
                    json!({
                        "contents": [{ "uri": "adb://device/installed_apps", "mimeType": "text/plain", "text": text }]
                    }),
                )
            }
            Err(e) => Response::err(id, -32000, e.to_string()),
        },

        "tools/list" => Response::ok(id, json!({ "tools": tool_definitions() })),

        "tools/call" => {
            let params = req.params.as_ref();
            let name = params
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params
                .and_then(|p| p.get("arguments"))
                .and_then(Value::as_object);
            match call_tool(name, args, executor) {
                Ok(text) => {
                    Response::ok(id, json!({ "content": [{ "type": "text", "text": text }] }))
                }
                Err(ToolError::BadArguments(msg)) => Response::err(id, -32602, msg),
                Err(ToolError::Failed(msg)) => Response::err(id, -32000, msg),
            }
        }

        method => Response::err(id, -32601, format!("method not found: {method}")),
    }
}

enum ToolError {
    /// The caller sent a wrong or missing argument.
    BadArguments(String),
    /// The tool ran and the phone or network refused.
    Failed(String),
}

impl From<AdbError> for ToolError {
    fn from(e: AdbError) -> Self {
        ToolError::Failed(e.to_string())
    }
}

type Args = serde_json::Map<String, Value>;

fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_apps",
            "description": "List the apps installed on the connected Android phone, with APK size and memory use.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "detailed": { "type": "boolean", "description": "Include APK size and RAM (default: true)" },
                    "include_system": { "type": "boolean", "description": "Include preinstalled system apps (default: false)" }
                }
            }
        },
        {
            "name": "device_info",
            "description": "Model, Android version, RAM and storage of the connected phone.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "app_details",
            "description": "Version, API levels, install dates, installer, size, memory and permissions of one installed app.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "package_name": { "type": "string", "description": "Android package name, e.g. com.example.app" }
                },
                "required": ["package_name"]
            }
        },
        {
            "name": "extract_apk",
            "description": "Pull an APK from the connected Android device to the local filesystem.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "package_name": { "type": "string", "description": "Android package name, e.g. com.example.app" },
                    "apk_path":     { "type": "string", "description": "Device-side APK path from list_apps" },
                    "destination":  { "type": "string", "description": "Local directory to save the APK (default: .)" }
                },
                "required": ["package_name", "apk_path"]
            }
        },
        {
            "name": "download_apk",
            "description": "Download an open-source app from F-Droid to the local filesystem. No phone needed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "package_name": { "type": "string", "description": "Package name on F-Droid, e.g. org.fdroid.fdroid" },
                    "destination":  { "type": "string", "description": "Local directory to save the APK (default: .)" }
                },
                "required": ["package_name"]
            }
        }
    ])
}

fn call_tool(
    name: &str,
    args: Option<&Args>,
    executor: &dyn AdbCommandExecutor,
) -> Result<String, ToolError> {
    let text = |key: &str| args.and_then(|a| a.get(key)).and_then(Value::as_str);
    let flag = |key: &str, default: bool| {
        args.and_then(|a| a.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(default)
    };
    let required = |key: &str| {
        text(key)
            .ok_or_else(|| ToolError::BadArguments(format!("missing required argument: {key}")))
    };

    match name {
        "list_apps" => {
            let detailed = flag("detailed", true);
            let lister = ListApps::new(executor);
            let apps = if detailed {
                lister.execute_detailed()?
            } else {
                lister.execute()?
            };
            let include_system = flag("include_system", false);
            let shown: Vec<_> = apps
                .iter()
                .filter(|a| include_system || !a.is_system())
                .collect();
            Ok(report::apps_table(&shown, detailed))
        }
        "device_info" => Ok(report::read_device_report(executor)?),
        "app_details" => Ok(report::read_app_report(
            executor,
            required("package_name")?,
        )?),
        "extract_apk" => {
            let (package, apk_path) = (required("package_name")?, required("apk_path")?);
            let dest = text("destination").unwrap_or(".");
            let saved = ExtractApk::new(executor).execute(apk_path, Path::new(dest), package)?;
            Ok(format!("APK saved to {}", saved.display()))
        }
        "download_apk" => {
            let package = required("package_name")?;
            let dest = text("destination").unwrap_or(".");
            let saved = download_apk(package, Path::new(dest))?;
            Ok(format!("APK saved to {}", saved.display()))
        }
        other => Err(ToolError::BadArguments(format!("unknown tool: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Answers the few adb commands the tools issue, like a phone with one user app.
    struct FakePhone;
    impl AdbCommandExecutor for FakePhone {
        fn run(&self, args: &[&str]) -> Result<String, AdbError> {
            let cmd = args.join(" ");
            Ok(if cmd.contains("pm list packages") {
                "package:/data/app/x/com.foo-1/base.apk=com.foo\n\
                 package:/system/app/Sys/Sys.apk=com.sys"
                    .into()
            } else if cmd.contains("stat -c") {
                "2097152 /data/app/x/com.foo-1/base.apk\n".into()
            } else if cmd.contains("dumpsys package com.foo") {
                "Packages:\n  Package [com.foo] (1):\n    versionName=3.1\n".into()
            } else if cmd.contains("ro.product.model") {
                "Pixel\n".into()
            } else if cmd.contains("ro.build.version.release") {
                "14\n".into()
            } else {
                String::new()
            })
        }
    }

    fn call(name: &str, args: Value) -> Result<String, ToolError> {
        call_tool(name, args.as_object(), &FakePhone)
    }

    #[test]
    fn every_tool_is_listed_and_callable() {
        let listed: Vec<String> = tool_definitions()
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            listed,
            [
                "list_apps",
                "device_info",
                "app_details",
                "extract_apk",
                "download_apk"
            ]
        );
    }

    #[test]
    fn list_apps_hides_system_apps_and_shows_size_by_default() {
        let text = call("list_apps", json!({})).ok().unwrap();
        assert!(text.contains("com.foo"));
        assert!(text.contains("2 MB"));
        assert!(!text.contains("com.sys"));
        let all = call("list_apps", json!({ "include_system": true }))
            .ok()
            .unwrap();
        assert!(all.contains("com.sys"));
    }

    #[test]
    fn device_info_reports_the_phone() {
        let text = call("device_info", json!({})).ok().unwrap();
        assert!(text.contains("Model:    Pixel"));
        assert!(text.contains("Android:  14"));
    }

    #[test]
    fn app_details_reads_one_app_and_rejects_unknown_ones() {
        let text = call("app_details", json!({ "package_name": "com.foo" }))
            .ok()
            .unwrap();
        assert_eq!(report::tests::value_of(&text, "Version:"), "3.1");
        assert_eq!(report::tests::value_of(&text, "APK size:"), "2 MB");
        assert!(matches!(
            call("app_details", json!({ "package_name": "com.missing" })),
            Err(ToolError::Failed(msg)) if msg.contains("not installed")
        ));
    }

    #[test]
    fn missing_arguments_and_unknown_tools_are_bad_arguments() {
        assert!(matches!(
            call("app_details", json!({})),
            Err(ToolError::BadArguments(_))
        ));
        assert!(matches!(
            call("extract_apk", json!({ "package_name": "a" })),
            Err(ToolError::BadArguments(_))
        ));
        assert!(matches!(
            call("format_disk", json!({})),
            Err(ToolError::BadArguments(_))
        ));
    }
}
