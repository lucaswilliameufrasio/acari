//! Install/remove the acari MCP server and the low-disk SessionStart hook in
//! AI coding agents (Claude Code, Codex, opencode).
//!
//! Every change is planned first (`Change`), shown to the user, and only then
//! applied. File edits keep key order and leave a `.acari-bak` copy; when the
//! agent ships a CLI for MCP servers (`claude mcp add`, `codex mcp add`) that
//! CLI is used instead of editing its files behind its back.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

pub const SERVER_NAME: &str = "acari";
/// Marker that identifies acari's hook among the user's other hooks.
const HOOK_MARKER: &str = "check --hook";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    ClaudeCode,
    Codex,
    OpenCode,
}

impl Agent {
    pub const ALL: [Agent; 3] = [Agent::ClaudeCode, Agent::Codex, Agent::OpenCode];

    pub fn name(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "Claude Code",
            Agent::Codex => "Codex",
            Agent::OpenCode => "opencode",
        }
    }

    fn binary(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude",
            Agent::Codex => "codex",
            Agent::OpenCode => "opencode",
        }
    }

    fn config_dir(self, home: &Path) -> PathBuf {
        match self {
            Agent::ClaudeCode => home.join(".claude"),
            Agent::Codex => home.join(".codex"),
            Agent::OpenCode => home.join(".config/opencode"),
        }
    }

    /// Whether the agent can inject context at session start.
    pub fn supports_hook(self) -> bool {
        matches!(self, Agent::ClaudeCode | Agent::Codex)
    }

    fn hooks_file(self, home: &Path) -> Option<PathBuf> {
        match self {
            Agent::ClaudeCode => Some(home.join(".claude/settings.json")),
            Agent::Codex => Some(home.join(".codex/hooks.json")),
            Agent::OpenCode => None,
        }
    }

    fn mcp_file(self, home: &Path) -> PathBuf {
        match self {
            Agent::ClaudeCode => home.join(".claude.json"),
            Agent::Codex => home.join(".codex/config.toml"),
            Agent::OpenCode => home.join(".config/opencode/opencode.json"),
        }
    }
}

/// Agents that look installed (binary on PATH or config directory present).
pub fn detect(home: &Path) -> Vec<Agent> {
    Agent::ALL
        .into_iter()
        .filter(|a| on_path(a.binary()) || a.config_dir(home).is_dir())
        .collect()
}

pub fn on_path(binary: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            let candidate = dir.join(binary);
            candidate.is_file()
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgentStatus {
    pub mcp: bool,
    pub hook: bool,
}

pub fn status(agent: Agent, home: &Path) -> AgentStatus {
    let mcp = match agent {
        Agent::ClaudeCode | Agent::OpenCode => read_json(&agent.mcp_file(home)).is_some_and(|v| {
            let key = if agent == Agent::ClaudeCode {
                "mcpServers"
            } else {
                "mcp"
            };
            v.get(key).and_then(|m| m.get(SERVER_NAME)).is_some()
        }),
        Agent::Codex => std::fs::read_to_string(agent.mcp_file(home))
            .ok()
            .and_then(|s| s.parse::<toml::Table>().ok())
            .is_some_and(|t| {
                t.get("mcp_servers")
                    .and_then(|m| m.get(SERVER_NAME))
                    .is_some()
            }),
    };
    let hook = agent
        .hooks_file(home)
        .and_then(|p| read_json(&p))
        .is_some_and(|v| has_hook(hooks_root(agent, &v)));
    AgentStatus { mcp, hook }
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Claude keeps hooks under `hooks` in settings.json; Codex's hooks.json has
/// the same `hooks` wrapper.
fn hooks_root(_agent: Agent, value: &Value) -> Option<&Value> {
    value.get("hooks")
}

fn has_hook(hooks: Option<&Value>) -> bool {
    hooks
        .and_then(|h| h.get("SessionStart"))
        .and_then(Value::as_array)
        .is_some_and(|groups| groups.iter().any(group_is_acari))
}

fn group_is_acari(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks.iter().any(|h| {
                h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| c.contains("acari") && c.contains(HOOK_MARKER))
            })
        })
}

pub use super::changes::Change;

fn exe_string(exe: &Path) -> String {
    exe.display().to_string()
}

pub fn hook_command(exe: &Path) -> String {
    format!(
        "{} {HOOK_MARKER}",
        crate::domain::insights::shell_quote(exe)
    )
}

/// Plan installing (or replacing) the MCP server entry.
pub fn plan_mcp_install(agent: Agent, home: &Path, exe: &Path) -> Result<Vec<Change>, String> {
    let exe_s = exe_string(exe);
    let already = status(agent, home).mcp;
    match agent {
        Agent::ClaudeCode | Agent::Codex if on_path(agent.binary()) => {
            let mut changes = Vec::new();
            if already {
                changes.extend(plan_mcp_remove(agent, home)?);
            }
            let args: Vec<String> = match agent {
                Agent::ClaudeCode => vec![
                    "mcp".into(),
                    "add".into(),
                    "--scope".into(),
                    "user".into(),
                    SERVER_NAME.into(),
                    "--".into(),
                    exe_s,
                    "mcp".into(),
                ],
                _ => vec![
                    "mcp".into(),
                    "add".into(),
                    SERVER_NAME.into(),
                    "--".into(),
                    exe_s,
                    "mcp".into(),
                ],
            };
            changes.push(Change::Run {
                program: agent.binary().into(),
                args,
                allow_failure: false,
            });
            Ok(changes)
        }
        Agent::ClaudeCode | Agent::Codex => Ok(vec![Change::Manual {
            instructions: format!(
                "{} CLI not found on PATH; add an MCP server named \"{SERVER_NAME}\" running `{exe_s} mcp`",
                agent.name()
            ),
        }]),
        Agent::OpenCode => {
            let path = agent.mcp_file(home);
            let content = edit_json_file(&path, |root| {
                let mcp = object_at(root, "mcp");
                mcp.insert(
                    SERVER_NAME.into(),
                    json!({ "type": "local", "command": [exe_s, "mcp"], "enabled": true }),
                );
            })?;
            Ok(vec![Change::WriteFile {
                path,
                content,
                summary: format!("+ mcp.{SERVER_NAME}"),
            }])
        }
    }
}

pub fn plan_mcp_remove(agent: Agent, home: &Path) -> Result<Vec<Change>, String> {
    if !status(agent, home).mcp {
        return Ok(Vec::new());
    }
    match agent {
        Agent::ClaudeCode | Agent::Codex if on_path(agent.binary()) => {
            let args: Vec<String> = if agent == Agent::ClaudeCode {
                vec![
                    "mcp".into(),
                    "remove".into(),
                    "--scope".into(),
                    "user".into(),
                    SERVER_NAME.into(),
                ]
            } else {
                vec!["mcp".into(), "remove".into(), SERVER_NAME.into()]
            };
            Ok(vec![Change::Run {
                program: agent.binary().into(),
                args,
                allow_failure: false,
            }])
        }
        Agent::ClaudeCode | Agent::Codex => Ok(vec![Change::Manual {
            instructions: format!(
                "remove the \"{SERVER_NAME}\" MCP server from {}",
                agent.mcp_file(home).display()
            ),
        }]),
        Agent::OpenCode => {
            let path = agent.mcp_file(home);
            let content = edit_json_file(&path, |root| {
                if let Some(mcp) = root.get_mut("mcp").and_then(Value::as_object_mut) {
                    mcp.shift_remove(SERVER_NAME);
                }
            })?;
            Ok(vec![Change::WriteFile {
                path,
                content,
                summary: format!("- mcp.{SERVER_NAME}"),
            }])
        }
    }
}

pub fn plan_hook_install(agent: Agent, home: &Path, exe: &Path) -> Result<Vec<Change>, String> {
    let Some(path) = agent.hooks_file(home) else {
        return Ok(vec![Change::Manual {
            instructions: format!(
                "{} has no session-start hook that can add context; the MCP server covers it",
                agent.name()
            ),
        }]);
    };
    let command = hook_command(exe);
    let content = edit_json_file(&path, |root| set_hook(root, Some(&command)))?;
    Ok(vec![Change::WriteFile {
        path,
        content,
        summary: format!("+ hooks.SessionStart: {command}"),
    }])
}

pub fn plan_hook_remove(agent: Agent, home: &Path) -> Result<Vec<Change>, String> {
    let Some(path) = agent.hooks_file(home) else {
        return Ok(Vec::new());
    };
    if !status(agent, home).hook {
        return Ok(Vec::new());
    }
    let content = edit_json_file(&path, |root| set_hook(root, None))?;
    Ok(vec![Change::WriteFile {
        path,
        content,
        summary: "- hooks.SessionStart (acari)".into(),
    }])
}

/// Remove any acari SessionStart hook and, when `command` is given, add one.
pub fn set_hook(root: &mut Map<String, Value>, command: Option<&str>) {
    let hooks = object_at(root, "hooks");
    let session = hooks
        .entry("SessionStart")
        .or_insert_with(|| Value::Array(Vec::new()));
    if !session.is_array() {
        *session = Value::Array(Vec::new());
    }
    let groups = session.as_array_mut().expect("array");
    groups.retain(|g| !group_is_acari(g));
    if let Some(command) = command {
        groups.push(json!({
            "matcher": "",
            "hooks": [{ "type": "command", "command": command }],
        }));
    }
    if groups.is_empty() {
        hooks.shift_remove("SessionStart");
    }
}

fn object_at<'a>(root: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let entry = root.entry(key).or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    entry.as_object_mut().expect("object")
}

/// Load a JSON object file (missing file = `{}`), apply `edit`, and return
/// the new pretty-printed content. Invalid JSON is refused, never replaced.
fn edit_json_file(
    path: &Path,
    edit: impl FnOnce(&mut Map<String, Value>),
) -> Result<String, String> {
    let mut root = match std::fs::read_to_string(path) {
        Ok(content) if content.trim().is_empty() => Map::new(),
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(Value::Object(map)) => map,
            Ok(_) => return Err(format!("{} is not a JSON object", path.display())),
            Err(e) => {
                return Err(format!(
                    "{} is not plain JSON ({e}); edit it by hand",
                    path.display()
                ));
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    edit(&mut root);
    let mut out = serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())?;
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn hook_is_added_once_and_keeps_other_hooks_and_key_order() {
        let mut root: Map<String, Value> = serde_json::from_str(
            r#"{"zeta":1,"hooks":{"SessionStart":[{"matcher":"","hooks":[{"type":"command","command":"other-tool hook"}]}],"Stop":[]},"alpha":2}"#,
        )
        .unwrap();
        set_hook(&mut root, Some("/bin/acari check --hook"));
        set_hook(&mut root, Some("/new/acari check --hook"));
        let groups = root["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(groups.len(), 2, "replaced, not duplicated");
        assert_eq!(groups[1]["hooks"][0]["command"], "/new/acari check --hook");
        let keys: Vec<&String> = root.keys().collect();
        assert_eq!(keys, ["zeta", "hooks", "alpha"]);

        set_hook(&mut root, None);
        let groups = root["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["hooks"][0]["command"], "other-tool hook");
    }

    #[test]
    fn removing_the_last_hook_drops_the_event() {
        let mut root = Map::new();
        set_hook(&mut root, Some("acari check --hook"));
        assert!(has_hook(root.get("hooks")));
        set_hook(&mut root, None);
        assert!(root["hooks"].get("SessionStart").is_none());
    }

    #[test]
    fn plans_and_applies_hook_and_opencode_mcp_in_a_fake_home() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        let exe = Path::new("/opt/acari/bin/acari");

        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::write(home.join(".claude/settings.json"), r#"{"model":"x"}"#).unwrap();
        for change in plan_hook_install(Agent::ClaudeCode, home, exe).unwrap() {
            change.apply().unwrap();
        }
        assert!(status(Agent::ClaudeCode, home).hook);
        assert!(home.join(".claude/settings.json.acari-bak").exists());
        let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
        assert!(settings.starts_with("{\n  \"model\""), "{settings}");

        for change in plan_mcp_install(Agent::OpenCode, home, exe).unwrap() {
            change.apply().unwrap();
        }
        assert!(status(Agent::OpenCode, home).mcp);
        for change in plan_mcp_remove(Agent::OpenCode, home).unwrap() {
            change.apply().unwrap();
        }
        assert!(!status(Agent::OpenCode, home).mcp);

        for change in plan_hook_remove(Agent::ClaudeCode, home).unwrap() {
            change.apply().unwrap();
        }
        assert!(!status(Agent::ClaudeCode, home).hook);
    }

    #[test]
    fn invalid_json_is_refused_not_overwritten() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join(".config/opencode/opencode.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ // comment\n}").unwrap();
        assert!(plan_mcp_install(Agent::OpenCode, home.path(), Path::new("/a")).is_err());
    }

    #[test]
    fn codex_status_reads_toml() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".codex")).unwrap();
        fs::write(
            home.path().join(".codex/config.toml"),
            "[mcp_servers.acari]\ncommand = \"acari\"\nargs = [\"mcp\"]\n",
        )
        .unwrap();
        assert!(status(Agent::Codex, home.path()).mcp);
    }
}
