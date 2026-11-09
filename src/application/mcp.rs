//! `acari mcp`: a Model Context Protocol server over stdio (newline-delimited
//! JSON-RPC 2.0). Agents get the same guidance a human gets from
//! `acari guide`, plus guarded cleanup tools that default to dry runs.
//!
//! stdout carries protocol messages only; diagnostics go to stderr.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::application::check;
use crate::application::cleaner::CleanMode;
use crate::application::commands::{prepare_targets, start_scan};
use crate::application::guide::{self, GuideOptions, GuideReport};
use crate::config::target_config::{self, TargetConfig};
use crate::domain::insights::{BuildKind, Category};
use crate::domain::{AppEvent, CleanTarget};
use crate::i18n::Language;
use crate::infrastructure::exec::parse_human_size;
use crate::infrastructure::explore::{self, ExploreOptions};
use crate::infrastructure::{cleaner as infra_cleaner, history};

/// Newest protocol revision this server implements.
const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
/// Findings per category returned by disk_guide unless a category is given.
const GUIDE_ITEMS_PER_CATEGORY: usize = 8;

const INSTRUCTIONS: &str = "acari explains what fills the user's disk and cleans it safely. \
Start with disk_status (instant). For a breakdown use disk_guide: it returns the last saved \
report when recent; refresh=true rescans the home folder and can take minutes. \
Cleanup tools (clean_targets, delete_build_dir) default to dry_run=true: show the user what \
would be freed and only call again with dry_run=false after the user explicitly agrees. \
Games, media, downloads and app data are never deleted by acari: relay the finding's hint.";

pub struct Server {
    lang: Language,
    runtime: Option<tokio::runtime::Handle>,
}

impl Server {
    pub fn new(lang: Language) -> Self {
        Self {
            lang,
            runtime: tokio::runtime::Handle::try_current().ok(),
        }
    }

    /// Serve until stdin closes.
    pub fn serve(&self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(response) = self.handle_line(&line) {
                writeln!(output, "{response}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// Handle one JSON-RPC message; `None` for notifications.
    pub fn handle_line(&self, line: &str) -> Option<Value> {
        let message: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                return Some(error_response(
                    Value::Null,
                    -32700,
                    &format!("parse error: {e}"),
                ));
            }
        };
        if let Value::Array(batch) = message {
            let responses: Vec<Value> = batch
                .into_iter()
                .filter_map(|m| self.handle_message(m))
                .collect();
            return (!responses.is_empty()).then_some(Value::Array(responses));
        }
        self.handle_message(message)
    }

    fn handle_message(&self, message: Value) -> Option<Value> {
        // Notifications (no id) never get a response, valid or not.
        let id = message.get("id").cloned()?;
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Some(error_response(
                id,
                -32600,
                "invalid request: missing method",
            ));
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(initialize_result(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => Ok(self.call_tool(&params)),
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, msg)) => error_response(id, code, &msg),
        })
    }

    fn call_tool(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let outcome = match name {
            "disk_status" => self.disk_status(),
            "disk_guide" => self.disk_guide(&args),
            "largest_paths" => largest_paths(&args),
            "list_targets" => Ok(list_targets()),
            "scan_targets" => self.scan_targets(&args),
            "clean_targets" => self.clean_targets(&args),
            "delete_build_dir" => delete_build_dir(&args),
            "cleanup_history" => Ok(cleanup_history(&args)),
            _ => Err(format!("unknown tool: {name}")),
        };
        match outcome {
            Ok(value) => json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                "structuredContent": value,
            }),
            Err(message) => json!({
                "content": [{ "type": "text", "text": message }],
                "isError": true,
            }),
        }
    }

    fn disk_status(&self) -> Result<Value, String> {
        let cfg = target_config::load_config();
        let result = check::measure(&cfg).ok_or("could not read disk usage")?;
        let mut value = serde_json::to_value(&result).map_err(|e| e.to_string())?;
        value["summary"] = json!(check::headline(&result, self.lang));
        value["thresholds"] = serde_json::to_value(cfg.alerts).unwrap_or_default();
        Ok(value)
    }

    fn disk_guide(&self, args: &Value) -> Result<Value, String> {
        let refresh = args
            .get("refresh")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_age = args
            .get("max_age_days")
            .and_then(Value::as_u64)
            .unwrap_or(7);
        let category = match args.get("category").and_then(Value::as_str) {
            Some(raw) => Some(guide::parse_category(raw).ok_or_else(|| {
                format!(
                    "unknown category {raw:?}; valid: {}",
                    guide::category_slugs().join(", ")
                )
            })?),
            None => None,
        };
        let saved = guide::load_saved_report();
        let (report, source) = match saved {
            Some(report) if !refresh && guide::report_age_days(&report) <= max_age => {
                (report, "saved")
            }
            _ => {
                let cfg = target_config::load_config();
                let opts = GuideOptions {
                    show_progress: false,
                    ..GuideOptions::default()
                };
                let report = guide::build_report(&opts, &cfg, self.lang);
                let _ = guide::save_report(&report);
                (report, "fresh")
            }
        };
        let age = guide::report_age_days(&report);
        let report = compact_report(report, category, self.lang);
        let mut value = serde_json::to_value(&report).map_err(|e| e.to_string())?;
        value["source"] = json!(source);
        value["age_days"] = json!(age);
        Ok(value)
    }

    fn block_on<F: std::future::Future>(&self, fut: F) -> Result<F::Output, String> {
        match &self.runtime {
            Some(handle) => Ok(handle.block_on(fut)),
            None => tokio::runtime::Runtime::new()
                .map(|rt| rt.block_on(fut))
                .map_err(|e| e.to_string()),
        }
    }

    fn scan(&self, targets: Vec<CleanTarget>, cfg: &TargetConfig) -> Result<Vec<Scanned>, String> {
        let excludes = cfg.scan.exclude_patterns.clone();
        let priority = cfg.scan.io_priority;
        let lookup: HashMap<String, CleanTarget> = targets
            .iter()
            .map(|t| (t.name.to_string(), t.clone()))
            .collect();
        self.block_on(async move {
            let (_tx, mut rx, handle) = start_scan(targets, excludes, priority, false);
            let mut out = Vec::new();
            while let Some(event) = rx.recv().await {
                match event {
                    AppEvent::TargetCompleted {
                        target_name,
                        total_bytes,
                        files_scanned,
                        scan_errors,
                        ..
                    } => {
                        if let Some(target) = lookup.get(&target_name) {
                            out.push(Scanned {
                                target: target.clone(),
                                bytes: total_bytes,
                                files: files_scanned,
                                errors: scan_errors,
                            });
                        }
                    }
                    AppEvent::ScanFinished => break,
                    _ => {}
                }
            }
            handle
                .await
                .map_err(|e| format!("scan worker failed: {e}"))?;
            Ok(out)
        })?
    }

    fn scan_targets(&self, args: &Value) -> Result<Value, String> {
        let cfg = target_config::load_config();
        let names = string_list(args.get("targets"));
        let targets = prepare_targets(&names, &[], &cfg.custom_targets);
        if targets.is_empty() {
            return Err("no targets matched; call list_targets for valid names".into());
        }
        let mut scanned = self.scan(targets, &cfg)?;
        scanned.sort_by_key(|s| std::cmp::Reverse(s.bytes));
        Ok(json!({
            "targets": scanned.iter().map(Scanned::to_json).collect::<Vec<_>>(),
            "note": "Estimates; nested targets may overlap. Clean with clean_targets (dry_run first).",
        }))
    }

    fn clean_targets(&self, args: &Value) -> Result<Value, String> {
        let names = string_list(args.get("targets"));
        if names.is_empty() {
            return Err("pass `targets`: names from list_targets or scan_targets".into());
        }
        let dry_run = args.get("dry_run").and_then(Value::as_bool).unwrap_or(true);
        let allow_dangerous = args
            .get("allow_dangerous")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let cfg = target_config::load_config();
        let targets = prepare_targets(&names, &[], &cfg.custom_targets);
        let mut refused = Vec::new();
        let mut allowed = Vec::new();
        for target in targets {
            if target.requires_sudo {
                refused.push(json!({
                    "target": target.name,
                    "reason": "needs administrator rights; ask the user to run `acari` in a terminal",
                }));
            } else if target.is_dangerous() && !allow_dangerous {
                refused.push(json!({
                    "target": target.name,
                    "reason": "marked dangerous (may delete data that does not regenerate); confirm with the user and pass allow_dangerous=true",
                }));
            } else {
                allowed.push(target);
            }
        }
        let unknown: Vec<&String> = names
            .iter()
            .filter(|n| {
                !allowed.iter().any(|t| t.name.eq_ignore_ascii_case(n))
                    && !refused.iter().any(|r| {
                        r["target"]
                            .as_str()
                            .is_some_and(|t| t.eq_ignore_ascii_case(n))
                    })
            })
            .collect();
        let scanned = if allowed.is_empty() {
            Vec::new()
        } else {
            self.scan(allowed, &cfg)?
        };
        let mode = if dry_run {
            CleanMode::DryRun
        } else {
            CleanMode::Execute
        };
        let mut results = Vec::new();
        let mut reclaimed_total = 0_u64;
        let mut errors_total = 0_u64;
        for s in &scanned {
            let r = infra_cleaner::clean_target(&s.target, s.bytes, s.files, mode);
            reclaimed_total = reclaimed_total.saturating_add(r.reclaimed_bytes);
            errors_total = errors_total.saturating_add(r.errors);
            results.push(json!({
                "target": s.target.name,
                "bytes": if dry_run { s.bytes } else { r.reclaimed_bytes },
                "errors": r.errors,
                "error_detail": r.error_detail,
            }));
        }
        if !dry_run && !scanned.is_empty() {
            history::append_entry(&format!(
                "{} | Clean completed (mcp) | targets={} reclaimed={reclaimed_total} errors={errors_total}",
                history::format_local_time(),
                scanned.len()
            ));
        }
        Ok(json!({
            "dry_run": dry_run,
            "results": results,
            "total_bytes": if dry_run { scanned.iter().map(|s| s.bytes).sum::<u64>() } else { reclaimed_total },
            "refused": refused,
            "unknown": unknown,
        }))
    }
}

struct Scanned {
    target: CleanTarget,
    bytes: u64,
    files: u64,
    errors: u64,
}

impl Scanned {
    fn to_json(&self) -> Value {
        json!({
            "target": self.target.name,
            "bytes": self.bytes,
            "files": self.files,
            "scan_errors": self.errors,
            "dangerous": self.target.is_dangerous(),
            "requires_sudo": self.target.requires_sudo,
        })
    }
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize_result(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);
    let version = if SUPPORTED_VERSIONS.contains(&requested) {
        requested
    } else {
        PROTOCOL_VERSION
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "acari", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// Trim each category to its biggest findings so the payload stays small.
fn compact_report(report: GuideReport, category: Option<Category>, lang: Language) -> GuideReport {
    if let Some(category) = category {
        return guide::only_category(report, category, lang);
    }
    let mut seen: HashMap<Category, usize> = HashMap::new();
    let mut report = report;
    report.findings.retain(|f| {
        let n = seen.entry(f.category).or_default();
        *n += 1;
        *n <= GUIDE_ITEMS_PER_CATEGORY
    });
    report
}

fn largest_paths(args: &Value) -> Result<Value, String> {
    let raw = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("pass `path`")?;
    let path = crate::domain::expand_tilde(raw);
    if !path.is_dir() {
        return Err(format!("not a directory: {}", path.display()));
    }
    let top = args.get("top").and_then(Value::as_u64).unwrap_or(20) as usize;
    let min = args
        .get("min_size")
        .and_then(Value::as_str)
        .unwrap_or("100MB");
    let min_bytes = parse_human_size(min).ok_or_else(|| format!("invalid min_size: {min}"))?;
    let data = explore::explore(
        &[path],
        ExploreOptions {
            big_file_min: min_bytes.max(1),
            keep_dir_min: min_bytes,
            io_priority: target_config::load_config().scan.io_priority,
        },
        |_| {},
    );
    let mut dirs: Vec<(&PathBuf, u64)> = data.dirs.iter().map(|(p, s)| (p, s.bytes)).collect();
    dirs.sort_by_key(|&(_, bytes)| std::cmp::Reverse(bytes));
    Ok(json!({
        "directories": dirs.iter().take(top).map(|(p, b)| json!({ "path": p, "bytes": b })).collect::<Vec<_>>(),
        "large_files": data.big_files.iter().take(top).map(|f| json!({ "path": f.path, "bytes": f.bytes })).collect::<Vec<_>>(),
        "scan_errors": data.errors,
        "note": "Directories aggregate their children (ancestors listed first). Same filesystem only.",
    }))
}

fn list_targets() -> Value {
    let cfg = target_config::load_config();
    let targets = prepare_targets(&[], &[], &cfg.custom_targets);
    json!({
        "targets": targets.iter().map(|t| json!({
            "name": t.name,
            "path": if t.is_command() { Value::Null } else { json!(t.path) },
            "description": t.description,
            "command": t.is_command(),
            "dangerous": t.is_dangerous(),
            "requires_sudo": t.requires_sudo,
            "custom": t.is_custom(),
        })).collect::<Vec<_>>(),
    })
}

/// Validate that `path` is a regenerable project build directory: a known
/// build folder name next to its project marker, inside the home folder.
fn validate_build_dir(path: &Path) -> Result<(PathBuf, BuildKind), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_dir() {
        return Err("not a directory (symlinks are refused)".into());
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let home = dirs::home_dir()
        .and_then(|h| h.canonicalize().ok())
        .ok_or("no home directory")?;
    if !path.starts_with(&home) || path == home {
        return Err("only build folders inside the home directory can be deleted".into());
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = path.parent().ok_or("no parent directory")?;
    let kind = BuildKind::detect(&name, |marker| parent.join(marker).is_file()).ok_or_else(|| {
        format!(
            "{} is not a recognised build folder (e.g. target/ next to Cargo.toml, node_modules/ next to package.json)",
            path.display()
        )
    })?;
    Ok((path, kind))
}

fn delete_build_dir(args: &Value) -> Result<Value, String> {
    let raw = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("pass `path` (a project_builds finding from disk_guide)")?;
    let dry_run = args.get("dry_run").and_then(Value::as_bool).unwrap_or(true);
    let (path, kind) = validate_build_dir(&crate::domain::expand_tilde(raw))?;
    let data = explore::explore(
        std::slice::from_ref(&path),
        ExploreOptions {
            big_file_min: u64::MAX,
            keep_dir_min: 0,
            io_priority: crate::config::target_config::IoPriority::Normal,
        },
        |_| {},
    );
    let bytes = data.dirs.get(&path).map_or(0, |s| s.bytes);
    if !dry_run {
        std::fs::remove_dir_all(&path).map_err(|e| format!("delete failed: {e}"))?;
        history::append_entry(&format!(
            "{} | Build folder deleted (mcp) | path={} reclaimed={bytes}",
            history::format_local_time(),
            path.display()
        ));
    }
    Ok(json!({
        "dry_run": dry_run,
        "path": path,
        "kind": kind,
        "bytes": bytes,
    }))
}

fn cleanup_history(args: &Value) -> Value {
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize;
    let entries = history::read_entries();
    let start = entries.len().saturating_sub(limit);
    json!({ "entries": &entries[start..] })
}

fn tool_definitions() -> Value {
    let categories = guide::category_slugs();
    json!([
        {
            "name": "disk_status",
            "title": "Disk status",
            "description": "Instant disk health: free space, alert level against the user's thresholds, and highlights from the last saved report. Call this first.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "disk_guide",
            "title": "Explain disk usage",
            "description": "Where the disk space went: findings by category (games, media, downloads, project builds, caches, toolchains, containers, VMs, large files, other folders) with risk, hint and suggested command, plus totals and next steps. Returns the saved report when younger than max_age_days; refresh=true rescans the home folder (can take minutes).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "refresh": { "type": "boolean", "description": "Rescan instead of using the saved report", "default": false },
                    "max_age_days": { "type": "integer", "minimum": 0, "default": 7 },
                    "category": { "type": "string", "enum": categories, "description": "Return every finding of one category" },
                },
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "largest_paths",
            "title": "Largest folders and files",
            "description": "du-style: the largest directories and files under a path (same filesystem, hard links once). Use it to drill into a finding.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to inspect (~ allowed)" },
                    "top": { "type": "integer", "minimum": 1, "default": 20 },
                    "min_size": { "type": "string", "default": "100MB" },
                },
                "required": ["path"],
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "list_targets",
            "title": "List cleanup targets",
            "description": "Caches and other locations acari knows how to clean (built-in and the user's custom targets), with danger and sudo flags.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "scan_targets",
            "title": "Measure cleanup targets",
            "description": "Measure how much space cleanup targets hold. Omit `targets` to measure all (can take a minute).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "targets": { "type": "array", "items": { "type": "string" }, "description": "Target names from list_targets" },
                },
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "clean_targets",
            "title": "Clean targets",
            "description": "Clean acari targets. dry_run defaults to true: report what would be freed. Only pass dry_run=false after the user agrees. Dangerous targets also need allow_dangerous=true; targets needing sudo are refused (the user must run acari in a terminal).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "targets": { "type": "array", "items": { "type": "string" } },
                    "dry_run": { "type": "boolean", "default": true },
                    "allow_dangerous": { "type": "boolean", "default": false },
                },
                "required": ["targets"],
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false },
        },
        {
            "name": "delete_build_dir",
            "title": "Delete a project build folder",
            "description": "Delete one regenerable build folder (e.g. target/ next to Cargo.toml, node_modules/ next to package.json) inside the home directory. Anything else is refused. dry_run defaults to true; pass dry_run=false only after the user agrees.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "dry_run": { "type": "boolean", "default": true },
                },
                "required": ["path"],
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false },
        },
        {
            "name": "cleanup_history",
            "title": "Cleanup history",
            "description": "Recent cleanups performed by acari (terminal, desktop and MCP).",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "minimum": 1, "default": 20 } },
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
    ])
}

/// Entry point for `acari mcp`.
pub fn run_mcp(lang: Language) -> anyhow::Result<()> {
    let server = Server::new(lang);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    server.serve(stdin.lock(), stdout.lock())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn call(server: &Server, line: &str) -> Value {
        server.handle_line(line).expect("response")
    }

    #[test]
    fn handshake_and_tool_listing() {
        let server = Server::new(Language::English);
        let init = call(
            &server,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#,
        );
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "acari");
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
        let list = call(&server, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"disk_guide"));
        assert!(names.contains(&"clean_targets"));
    }

    #[test]
    fn unknown_methods_and_bad_json_are_errors() {
        let server = Server::new(Language::English);
        let r = call(&server, r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#);
        assert_eq!(r["error"]["code"], -32601);
        let r = call(&server, "{not json");
        assert_eq!(r["error"]["code"], -32700);
        let r = call(
            &server,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#,
        );
        assert_eq!(r["result"]["isError"], true);
    }

    #[test]
    fn requests_without_method_get_an_invalid_request_error() {
        let server = Server::new(Language::English);
        let r = call(&server, r#"{"jsonrpc":"2.0","id":9}"#);
        assert_eq!(r["error"]["code"], -32600);
        assert!(server.handle_line(r#"{"jsonrpc":"2.0"}"#).is_none());
    }

    #[test]
    fn disk_status_tool_returns_structured_content() {
        let server = Server::new(Language::English);
        let r = call(
            &server,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"disk_status","arguments":{}}}"#,
        );
        assert!(
            r["result"]["structuredContent"]["status"]["total"]
                .as_u64()
                .unwrap()
                > 0
        );
    }

    #[test]
    fn build_dir_validation_requires_marker_and_refuses_other_dirs() {
        let home = dirs::home_dir().unwrap().canonicalize().unwrap();
        let temp = tempfile::tempdir_in(&home).unwrap();
        let project = temp.path().join("proj");
        fs::create_dir_all(project.join("target/debug")).unwrap();
        fs::create_dir_all(project.join("src")).unwrap();
        assert!(
            validate_build_dir(&project.join("target")).is_err(),
            "no Cargo.toml yet"
        );
        fs::write(project.join("Cargo.toml"), "[package]").unwrap();
        let (path, kind) = validate_build_dir(&project.join("target")).unwrap();
        assert_eq!(kind, BuildKind::Cargo);
        assert!(path.ends_with("proj/target"));
        assert!(validate_build_dir(&project.join("src")).is_err());

        let outside = tempfile::tempdir().unwrap();
        if !outside.path().canonicalize().unwrap().starts_with(&home) {
            fs::write(outside.path().join("Cargo.toml"), "").unwrap();
            fs::create_dir(outside.path().join("target")).unwrap();
            assert!(validate_build_dir(&outside.path().join("target")).is_err());
        }
    }

    #[test]
    fn delete_build_dir_defaults_to_dry_run() {
        let home = dirs::home_dir().unwrap().canonicalize().unwrap();
        let temp = tempfile::tempdir_in(&home).unwrap();
        let project = temp.path().join("app");
        fs::create_dir_all(project.join("node_modules/x")).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        fs::write(project.join("node_modules/x/i.js"), vec![1u8; 10_000]).unwrap();
        let target = project.join("node_modules");

        let dry = delete_build_dir(&json!({ "path": target })).unwrap();
        assert_eq!(dry["dry_run"], true);
        assert!(target.exists());
        assert!(dry["bytes"].as_u64().unwrap() >= 10_000);
    }
}
