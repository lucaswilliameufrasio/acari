//! `acari setup`: an interactive, re-runnable wizard. Each step explains what
//! it configures, shows the current state and proposes sensible defaults, so
//! nobody has to learn the individual `acari project ...` subcommands.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::target_config::{self, TargetConfig};
use crate::domain::insights::display_path;
use crate::i18n::Language;

/// Steps the wizard knows. Running `acari setup <step>` runs only that one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SetupStep {
    /// Folders where your code lives (enables `acari project`)
    Projects,
}

impl SetupStep {
    pub const ALL: &'static [SetupStep] = &[SetupStep::Projects];

    fn title(self, lang: Language) -> &'static str {
        match self {
            SetupStep::Projects => tr(lang, "Pastas de projetos", "Project folders"),
        }
    }
}

/// Input/output used by the wizard, swappable in tests.
pub trait Prompter {
    /// Ask a question; `None` means "no interactive answer available".
    fn ask(&mut self, question: &str) -> Option<String>;
    fn say(&mut self, line: &str);
}

/// Terminal prompter. Without a TTY on stdin every question takes its
/// default, so `acari setup` stays scriptable.
pub struct TerminalPrompter {
    interactive: bool,
}

impl TerminalPrompter {
    pub fn new(assume_yes: bool) -> Self {
        Self {
            interactive: !assume_yes && std::io::stdin().is_terminal(),
        }
    }
}

impl Prompter for TerminalPrompter {
    fn ask(&mut self, question: &str) -> Option<String> {
        if !self.interactive {
            return None;
        }
        print!("{question} ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_string()),
        }
    }

    fn say(&mut self, line: &str) {
        println!("{line}");
    }
}

fn tr(lang: Language, pt: &'static str, en: &'static str) -> &'static str {
    match lang {
        Language::Portuguese => pt,
        Language::English => en,
    }
}

/// Parse a yes/no answer; empty or unrecognised input yields `default`.
pub fn parse_yes_no(answer: Option<&str>, default: bool) -> bool {
    match answer.map(|a| a.trim().to_lowercase()).as_deref() {
        Some("s" | "sim" | "y" | "yes") => true,
        Some("n" | "nao" | "não" | "no") => false,
        _ => default,
    }
}

fn confirm(p: &mut dyn Prompter, question: &str, default: bool, lang: Language) -> bool {
    let suffix = match (default, lang) {
        (true, Language::Portuguese) => "[S/n]",
        (false, Language::Portuguese) => "[s/N]",
        (true, Language::English) => "[Y/n]",
        (false, Language::English) => "[y/N]",
    };
    let answer = p.ask(&format!("{question} {suffix}"));
    parse_yes_no(answer.as_deref(), default)
}

pub struct SetupContext<'a> {
    pub cfg: TargetConfig,
    pub home: PathBuf,
    pub lang: Language,
    pub prompter: &'a mut dyn Prompter,
    pub changed: bool,
}

/// Run the given steps (all when empty) and save the config when it changed.
pub fn run_setup(
    steps: &[SetupStep],
    prompter: &mut dyn Prompter,
    lang: Language,
) -> Result<TargetConfig> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let mut ctx = SetupContext {
        cfg: target_config::load_config(),
        home,
        lang,
        prompter,
        changed: false,
    };
    run_steps(&mut ctx, steps)?;
    if ctx.changed {
        target_config::save_config(&ctx.cfg)?;
        let line = match lang {
            Language::Portuguese => format!(
                "Configuração salva em {}.",
                target_config::config_path().display()
            ),
            Language::English => format!(
                "Configuration saved to {}.",
                target_config::config_path().display()
            ),
        };
        ctx.prompter.say(&line);
    } else {
        ctx.prompter
            .say(tr(lang, "Nada mudou.", "Nothing changed."));
    }
    ctx.prompter.say(tr(
        lang,
        "Próximo passo: `acari guide` mostra o que está ocupando o disco. Rode `acari setup` de novo quando quiser ajustar.",
        "Next: `acari guide` shows what is filling your disk. Run `acari setup` again whenever you want to adjust.",
    ));
    Ok(ctx.cfg)
}

pub fn run_steps(ctx: &mut SetupContext<'_>, steps: &[SetupStep]) -> Result<()> {
    let steps = if steps.is_empty() {
        SetupStep::ALL
    } else {
        steps
    };
    let lang = ctx.lang;
    if steps.len() > 1 || steps == SetupStep::ALL {
        ctx.prompter.say(tr(
            lang,
            "Vamos configurar o acari. Enter aceita a opção em maiúscula.",
            "Let's set up acari. Press Enter to accept the capitalised option.",
        ));
    }
    for (i, step) in steps.iter().enumerate() {
        ctx.prompter.say("");
        ctx.prompter
            .say(&format!("[{}/{}] {}", i + 1, steps.len(), step.title(lang)));
        match step {
            SetupStep::Projects => step_projects(ctx)?,
        }
    }
    ctx.prompter.say("");
    Ok(())
}

fn step_projects(ctx: &mut SetupContext<'_>) -> Result<()> {
    let lang = ctx.lang;
    ctx.prompter.say(tr(
        lang,
        "  O acari procura builds (target/, node_modules/, .venv/...) dentro dessas pastas para `acari project`.",
        "  acari looks for builds (target/, node_modules/, .venv/...) inside these folders for `acari project`.",
    ));

    let current = ctx.cfg.project_scan.roots.clone();
    if current.is_empty() {
        ctx.prompter.say(tr(
            lang,
            "  Nenhuma configurada ainda.",
            "  None configured yet.",
        ));
    } else {
        ctx.prompter
            .say(tr(lang, "  Configuradas:", "  Configured:"));
        for root in &current {
            let shown = display_path(Path::new(root), &ctx.home);
            ctx.prompter.say(&format!("    {shown}"));
        }
        let mut keep = Vec::new();
        for root in &current {
            let shown = display_path(Path::new(root), &ctx.home);
            let question = match lang {
                Language::Portuguese => format!("  Manter {shown}?"),
                Language::English => format!("  Keep {shown}?"),
            };
            if confirm(ctx.prompter, &question, true, lang) {
                keep.push(root.clone());
            } else {
                ctx.changed = true;
            }
        }
        ctx.cfg.project_scan.roots = keep;
    }

    let suggestions: Vec<PathBuf> = target_config::suggested_project_roots(&ctx.home)
        .into_iter()
        .filter(|path| {
            !ctx.cfg
                .project_scan
                .roots
                .iter()
                .any(|r| Path::new(r) == path.as_path())
        })
        .collect();
    for path in suggestions {
        let shown = display_path(&path, &ctx.home);
        let question = match lang {
            Language::Portuguese => format!("  Adicionar {shown}?"),
            Language::English => format!("  Add {shown}?"),
        };
        if confirm(ctx.prompter, &question, true, lang)
            && ctx.cfg.add_project_root(&path.display().to_string())?
        {
            ctx.changed = true;
            ctx.prompter.say(&format!("    + {shown}"));
        }
    }

    while let Some(answer) = ctx.prompter.ask(tr(
        lang,
        "  Outra pasta (Enter para continuar):",
        "  Another folder (Enter to continue):",
    )) {
        if answer.is_empty() {
            break;
        }
        match ctx.cfg.add_project_root(&answer) {
            Ok(true) => {
                ctx.changed = true;
                ctx.prompter.say(&format!("    + {answer}"));
            }
            Ok(false) => {
                ctx.prompter
                    .say(tr(lang, "    Já estava na lista.", "    Already listed."))
            }
            Err(e) => ctx.prompter.say(&format!("    ! {e}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;

    use super::*;

    struct Scripted {
        answers: VecDeque<Option<String>>,
        said: Vec<String>,
    }

    impl Scripted {
        fn new(answers: &[Option<&str>]) -> Self {
            Self {
                answers: answers.iter().map(|a| a.map(str::to_string)).collect(),
                said: Vec::new(),
            }
        }
    }

    impl Prompter for Scripted {
        fn ask(&mut self, question: &str) -> Option<String> {
            self.said.push(question.to_string());
            self.answers.pop_front().flatten()
        }
        fn say(&mut self, line: &str) {
            self.said.push(line.to_string());
        }
    }

    fn ctx<'a>(home: &Path, prompter: &'a mut dyn Prompter) -> SetupContext<'a> {
        SetupContext {
            cfg: TargetConfig::default(),
            home: home.to_path_buf(),
            lang: Language::English,
            prompter,
            changed: false,
        }
    }

    fn fixture_home() -> tempfile::TempDir {
        #[cfg(target_os = "macos")]
        let temp = tempfile::tempdir_in("/tmp").unwrap();
        #[cfg(not(target_os = "macos"))]
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("projects")).unwrap();
        fs::create_dir(temp.path().join("code")).unwrap();
        temp
    }

    #[test]
    fn parses_yes_no_in_both_languages() {
        assert!(parse_yes_no(Some("sim"), false));
        assert!(parse_yes_no(Some("Y"), false));
        assert!(!parse_yes_no(Some("não"), true));
        assert!(!parse_yes_no(Some("no"), true));
        assert!(parse_yes_no(Some(""), true));
        assert!(!parse_yes_no(None, false));
    }

    #[test]
    fn accepts_suggestions_by_default_when_not_interactive() {
        let home = fixture_home();
        let mut p = Scripted::new(&[]);
        let mut c = ctx(home.path(), &mut p);
        run_steps(&mut c, &[SetupStep::Projects]).unwrap();
        assert!(c.changed);
        assert_eq!(c.cfg.project_scan.roots.len(), 2);
    }

    #[test]
    fn honours_declined_suggestions_and_extra_folders() {
        let home = fixture_home();
        let extra = home.path().join("work");
        fs::create_dir(&extra).unwrap();
        // projects: no, code: yes, then one extra folder, then finish.
        let extra_str = extra.display().to_string();
        let mut p = Scripted::new(&[Some("n"), Some(""), Some(&extra_str), Some("")]);
        let mut c = ctx(home.path(), &mut p);
        run_steps(&mut c, &[SetupStep::Projects]).unwrap();
        let roots = &c.cfg.project_scan.roots;
        assert_eq!(roots.len(), 2, "{roots:?}");
        assert!(roots.iter().any(|r| r.ends_with("code")));
        assert!(roots.iter().any(|r| r.ends_with("work")));
        assert!(!roots.iter().any(|r| r.ends_with("projects")));
    }

    #[test]
    fn can_remove_existing_roots() {
        let home = fixture_home();
        let mut p = Scripted::new(&[Some("n"), Some("n"), Some("n"), Some("")]);
        let mut c = ctx(home.path(), &mut p);
        c.cfg
            .add_project_root(&home.path().join("projects").display().to_string())
            .unwrap();
        run_steps(&mut c, &[SetupStep::Projects]).unwrap();
        assert!(c.cfg.project_scan.roots.is_empty());
        assert!(c.changed);
    }
}
