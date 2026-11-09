//! Desktop notifications (notify-send on Linux, osascript on macOS).

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Normal,
    Critical,
}

/// Show a desktop notification. Errors mean no notifier is available.
pub fn send(title: &str, body: &str, urgency: Urgency) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = urgency;
        let script = format!(
            "display notification {} with title {}",
            applescript_string(body),
            applescript_string(title)
        );
        return run(Command::new("osascript").args(["-e", &script]));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let level = match urgency {
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        };
        run(Command::new("notify-send").args(["-a", "acari", "-u", level, title, body]))
    }
}

fn run(cmd: &mut Command) -> Result<(), String> {
    match cmd.output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn applescript_string(raw: &str) -> String {
    format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::applescript_string;

    #[test]
    fn escapes_applescript_strings() {
        assert_eq!(applescript_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }
}
