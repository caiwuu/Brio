//! 一次性的 sidecar 调用：detect-tz / derive-fp。
//!
//! 这些命令 spawn 一个短命的 Python 子进程，等其退出再返回结果。
//! 不纳入 [`crate::browser_supervisor::BrowserSupervisor`]，也不需要双向 stdin。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Deserialize)]
pub struct DetectTzArgs {
    pub proxy_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DetectTzResult {
    pub timezone: String,
    pub detected: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeriveFpArgs {
    pub proxy_url: Option<String>,
    pub timezone: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeriveFpResult {
    pub fingerprint_id: String,
}

async fn run_sidecar(app: &AppHandle, args: Vec<String>) -> Result<String, String> {
    let shell = app.shell();
    let output = shell
        .sidecar("brio-py")
        .map_err(|e| e.to_string())?
        .args(args)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "sidecar failed (code={:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn find_event<'a>(stdout: &'a str, name: &str) -> Option<Value> {
    for raw in stdout.lines() {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(s) {
            if v.get("event").and_then(|e| e.as_str()) == Some(name) {
                return Some(v);
            }
        }
    }
    None
}

/// 探测时区（可复用：Tauri command 与 MCP 工具共用）。
pub async fn detect_timezone_inner(
    app: &AppHandle,
    proxy_url: Option<String>,
) -> Result<DetectTzResult, String> {
    let mut cmd_args: Vec<String> = vec!["detect-tz".into()];
    if let Some(p) = proxy_url.as_ref().filter(|s| !s.is_empty()) {
        cmd_args.push("--proxy-url".into());
        cmd_args.push(p.clone());
    }
    let stdout = run_sidecar(app, cmd_args).await?;
    let v = find_event(&stdout, "tz")
        .ok_or_else(|| "detect-tz: no tz event in output".to_string())?;
    Ok(DetectTzResult {
        timezone: v
            .get("timezone")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string(),
        detected: v.get("detected").and_then(|d| d.as_bool()).unwrap_or(false),
    })
}

/// 根据 (proxy, tz, lang) 派生稳定 fingerprint_id（可复用）。
pub async fn derive_fingerprint_id_inner(
    app: &AppHandle,
    proxy_url: Option<String>,
    timezone: Option<String>,
    language: Option<String>,
) -> Result<String, String> {
    let cmd_args: Vec<String> = vec![
        "derive-fp".into(),
        "--proxy-url".into(),
        proxy_url.unwrap_or_default(),
        "--timezone".into(),
        timezone.unwrap_or_default(),
        "--language".into(),
        language.unwrap_or_default(),
    ];
    let stdout = run_sidecar(app, cmd_args).await?;
    let v = find_event(&stdout, "fp")
        .ok_or_else(|| "derive-fp: no fp event in output".to_string())?;
    Ok(v.get("fingerprint_id")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string())
}

#[tauri::command]
pub async fn detect_timezone(
    app: AppHandle,
    args: DetectTzArgs,
) -> Result<DetectTzResult, String> {
    detect_timezone_inner(&app, args.proxy_url).await
}

#[tauri::command]
pub async fn derive_fingerprint_id(
    app: AppHandle,
    args: DeriveFpArgs,
) -> Result<DeriveFpResult, String> {
    let fingerprint_id =
        derive_fingerprint_id_inner(&app, args.proxy_url, args.timezone, args.language).await?;
    Ok(DeriveFpResult { fingerprint_id })
}
