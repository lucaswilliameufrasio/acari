//! Disk insights: turn a raw filesystem walk into an explained, prioritised
//! list of findings ("where did my disk go, and what can I do about it?").
//!
//! This module is pure: the walk happens in `infrastructure::explore`, the
//! well-known directories are resolved by the caller, and this code only
//! classifies and assembles.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::i18n::Language;

const SECS_PER_DAY: u64 = 86_400;

/// Aggregated (recursive) statistics of one directory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirStat {
    /// Allocated bytes of every file below this directory.
    pub bytes: u64,
    pub files: u64,
    /// Newest file modification time below this directory (unix seconds).
    pub newest: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigFile {
    pub path: PathBuf,
    pub bytes: u64,
    /// Modification time (unix seconds).
    pub modified: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildKind {
    Cargo,
    Node,
    Gradle,
    Python,
    Dart,
    Zig,
    Elixir,
    Swift,
}

impl BuildKind {
    /// Directory name, plus marker files of which at least one must sit next
    /// to it for the directory to count as a regenerable build output.
    pub const RULES: &'static [(&'static str, BuildKind, &'static [&'static str])] = &[
        ("target", BuildKind::Cargo, &["Cargo.toml"]),
        ("node_modules", BuildKind::Node, &["package.json"]),
        (".next", BuildKind::Node, &["package.json"]),
        (".nuxt", BuildKind::Node, &["package.json"]),
        (".turbo", BuildKind::Node, &["package.json"]),
        (".svelte-kit", BuildKind::Node, &["package.json"]),
        (".parcel-cache", BuildKind::Node, &["package.json"]),
        (
            ".angular",
            BuildKind::Node,
            &["package.json", "angular.json"],
        ),
        (
            ".gradle",
            BuildKind::Gradle,
            &[
                "build.gradle",
                "build.gradle.kts",
                "settings.gradle",
                "settings.gradle.kts",
            ],
        ),
        (
            "build",
            BuildKind::Gradle,
            &["build.gradle", "build.gradle.kts"],
        ),
        ("build", BuildKind::Dart, &["pubspec.yaml"]),
        (".dart_tool", BuildKind::Dart, &["pubspec.yaml"]),
        (
            ".venv",
            BuildKind::Python,
            &["pyproject.toml", "requirements.txt", "setup.py", "uv.lock"],
        ),
        (
            "venv",
            BuildKind::Python,
            &["pyproject.toml", "requirements.txt", "setup.py"],
        ),
        (".tox", BuildKind::Python, &["tox.ini", "pyproject.toml"]),
        (".zig-cache", BuildKind::Zig, &["build.zig"]),
        ("zig-out", BuildKind::Zig, &["build.zig"]),
        ("_build", BuildKind::Elixir, &["mix.exs"]),
        (".build", BuildKind::Swift, &["Package.swift"]),
    ];

    /// Return the build kind when `name` is a build directory whose parent
    /// holds one of the expected marker files.
    pub fn detect(name: &str, has_marker: impl Fn(&str) -> bool) -> Option<BuildKind> {
        Self::RULES
            .iter()
            .filter(|(dir, _, _)| *dir == name)
            .find(|(_, _, markers)| markers.iter().any(|m| has_marker(m)))
            .map(|(_, kind, _)| *kind)
    }

    fn clean_command(self, build_dir: &Path) -> String {
        let project = build_dir.parent().unwrap_or(build_dir);
        match self {
            BuildKind::Cargo => format!(
                "cargo clean --manifest-path {}",
                shell_quote(&project.join("Cargo.toml"))
            ),
            _ => format!("rm -rf {}", shell_quote(build_dir)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDir {
    pub path: PathBuf,
    pub kind: BuildKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// Repository that owns the worktree (where `git worktree` must run).
    pub main_repo: Option<PathBuf>,
}

/// Everything the explorer learned from walking the scan roots.
#[derive(Debug, Default)]
pub struct ExploreData {
    pub roots: Vec<PathBuf>,
    /// Recursive totals per directory (small directories may be pruned).
    pub dirs: HashMap<PathBuf, DirStat>,
    pub big_files: Vec<BigFile>,
    pub build_dirs: Vec<BuildDir>,
    /// Linked git worktrees (directories whose `.git` is a file pointing to
    /// a main repository's `worktrees/` folder).
    pub worktrees: Vec<Worktree>,
    pub errors: u64,
}

/// Well-known locations resolved by the caller (platform/XDG aware).
#[derive(Debug, Clone, Default)]
pub struct KnownDirs {
    pub home: PathBuf,
    pub videos: Option<PathBuf>,
    pub music: Option<PathBuf>,
    pub pictures: Option<PathBuf>,
    pub downloads: Option<PathBuf>,
    pub temp: Option<PathBuf>,
}

/// A cleanup target acari already knows how to clean.
#[derive(Debug, Clone)]
pub struct KnownTarget {
    pub name: String,
    pub path: PathBuf,
    pub description: String,
    pub dangerous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Games,
    Media,
    Downloads,
    ProjectBuilds,
    Worktrees,
    DevCaches,
    Toolchains,
    Containers,
    VirtualMachines,
    LargeFiles,
    Temp,
    Trash,
    Other,
}

impl Category {
    pub fn label(self, lang: Language) -> &'static str {
        let (pt, en) = match self {
            Category::Games => ("Jogos", "Games"),
            Category::Media => ("Mídia", "Media"),
            Category::Downloads => ("Downloads", "Downloads"),
            Category::ProjectBuilds => ("Builds de projetos", "Project builds"),
            Category::Worktrees => ("Worktrees git", "Git worktrees"),
            Category::DevCaches => ("Caches de desenvolvimento", "Developer caches"),
            Category::Toolchains => ("Toolchains e SDKs", "Toolchains and SDKs"),
            Category::Containers => ("Containers", "Containers"),
            Category::VirtualMachines => ("Máquinas virtuais e emuladores", "VMs and emulators"),
            Category::LargeFiles => ("Arquivos grandes", "Large files"),
            Category::Temp => ("Arquivos temporários", "Temporary files"),
            Category::Trash => ("Lixeira", "Trash"),
            Category::Other => ("Outras pastas grandes", "Other large folders"),
        };
        tr(lang, pt, en)
    }
}

/// How careful the user must be before reclaiming a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Regenerates on its own (caches, build outputs); costs only time.
    Safe,
    /// Managed by an application: remove it from inside that app.
    App,
    /// User data or unknown content: look before deleting.
    Review,
}

impl Risk {
    pub fn label(self, lang: Language) -> &'static str {
        match self {
            Risk::Safe => tr(lang, "regenera sozinho", "regenerates"),
            Risk::App => tr(lang, "remova pelo app", "remove via app"),
            Risk::Review => tr(lang, "revise antes", "review first"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Clean with an acari built-in or custom target.
    AcariTarget { target: String },
    /// Run a shell command.
    Command { command: String },
    /// Open an application and remove content from there.
    UseApp { app: String },
    /// Inspect manually; nothing automatic is suggested.
    Review,
}

impl Action {
    /// Copy-pastable command, when there is one.
    pub fn command_line(&self) -> Option<String> {
        match self {
            Action::AcariTarget { target } => Some(format!("acari -t {}", shell_quote_str(target))),
            Action::Command { command } => Some(command.clone()),
            Action::UseApp { .. } | Action::Review => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detail {
    pub name: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub path: PathBuf,
    /// Bytes attributed to this finding. For `Other` folders this is only the
    /// part not already explained by a more specific finding inside it.
    pub bytes: u64,
    pub files: u64,
    pub category: Category,
    pub label: String,
    pub risk: Risk,
    pub action: Action,
    pub hint: String,
    /// Days since anything below this path was modified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_days: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<Detail>,
}

#[derive(Debug, Clone, Copy)]
pub struct InsightOptions {
    /// Minimum size for a folder-level finding.
    pub min_bytes: u64,
    /// Minimum size for a single project build directory.
    pub min_build_bytes: u64,
    /// Builds and worktrees idle at least this long are flagged as stale.
    pub stale_days: u64,
    /// Current time (unix seconds), injected for deterministic tests.
    pub now: u64,
}

impl Default for InsightOptions {
    fn default() -> Self {
        Self {
            min_bytes: 1_000_000_000,
            min_build_bytes: 100_000_000,
            stale_days: 30,
            now: 0,
        }
    }
}

struct Location {
    path: PathBuf,
    category: Category,
    label: String,
    risk: Risk,
    action: Action,
    hint: String,
    /// Sub-folder whose children make the most useful breakdown.
    details_from: Option<PathBuf>,
}

fn known_locations(known: &KnownDirs, lang: Language) -> Vec<Location> {
    let home = &known.home;
    let mut out = Vec::new();
    let mut add = |path: PathBuf,
                   category: Category,
                   label: &str,
                   risk: Risk,
                   action: Action,
                   hint: String,
                   details_from: Option<PathBuf>| {
        out.push(Location {
            path,
            category,
            label: label.to_string(),
            risk,
            action,
            hint,
            details_from,
        });
    };

    // Games: libraries managed by launchers.
    let use_steam = || Action::UseApp {
        app: "Steam".into(),
    };
    let steam_hint = tr(
        lang,
        "Desinstale os jogos que não joga pela biblioteca do Steam (o Steam remove tudo certinho).",
        "Uninstall games you no longer play from the Steam library (Steam removes them cleanly).",
    )
    .to_string();
    for rel in [
        ".local/share/Steam",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        "Library/Application Support/Steam",
    ] {
        let path = home.join(rel);
        let common = path.join("steamapps/common");
        add(
            path,
            Category::Games,
            "Steam",
            Risk::App,
            use_steam(),
            steam_hint.clone(),
            Some(common),
        );
    }
    let launcher_hint = tr(
        lang,
        "Desinstale pelo launcher (Lutris, Heroic, Bottles, Battle.net...) para não deixar prefixos órfãos.",
        "Uninstall from the launcher (Lutris, Heroic, Bottles, Battle.net...) to avoid orphaned prefixes.",
    )
    .to_string();
    for (rel, label) in [
        ("Games", "Games"),
        (".local/share/lutris", "Lutris"),
        (".var/app/com.heroicgameslauncher.hgl", "Heroic"),
        (".local/share/bottles", "Bottles"),
        (".var/app/com.usebottles.bottles", "Bottles"),
    ] {
        add(
            home.join(rel),
            Category::Games,
            label,
            Risk::App,
            Action::UseApp { app: label.into() },
            launcher_hint.clone(),
            None,
        );
    }

    // Media and downloads: user data, never automatic.
    let media_hint = tr(
        lang,
        "São seus arquivos: mova o que quer guardar para um HD externo/nuvem e apague o resto.",
        "These are your files: move what you want to keep to external/cloud storage and delete the rest.",
    )
    .to_string();
    for (dir, label) in [
        (&known.videos, tr(lang, "Vídeos", "Videos")),
        (&known.music, tr(lang, "Música", "Music")),
        (&known.pictures, tr(lang, "Imagens", "Pictures")),
    ] {
        if let Some(dir) = dir {
            add(
                dir.clone(),
                Category::Media,
                label,
                Risk::Review,
                Action::Review,
                media_hint.clone(),
                None,
            );
        }
    }
    if let Some(dir) = &known.downloads {
        add(
            dir.clone(),
            Category::Downloads,
            "Downloads",
            Risk::Review,
            Action::Review,
            tr(
                lang,
                "Instaladores, ISOs e arquivos compactados já usados costumam ser os maiores vilões.",
                "Installers, ISOs and archives you already used are usually the biggest offenders.",
            )
            .to_string(),
            None,
        );
    }

    // Containers.
    let docker_hint = tr(
        lang,
        "Veja o que ocupa com `docker system df -v`; imagens e build cache regeneram, volumes podem ter dados de bancos.",
        "Inspect with `docker system df -v`; images and build cache regenerate, volumes may hold database data.",
    )
    .to_string();
    for (rel, label) in [
        (".local/share/docker", "Docker (rootless)"),
        (".local/share/containers", "Podman"),
        ("Library/Containers/com.docker.docker", "Docker Desktop"),
        (".orbstack", "OrbStack"),
        (".colima", "Colima"),
    ] {
        add(
            home.join(rel),
            Category::Containers,
            label,
            Risk::Review,
            Action::Command {
                command: "docker system df -v".into(),
            },
            docker_hint.clone(),
            None,
        );
    }

    // Virtual machines and emulators.
    let vm_hint = tr(
        lang,
        "Apague as VMs/emuladores que não usa mais pelo próprio gerenciador.",
        "Delete VMs/emulators you no longer use from their manager.",
    )
    .to_string();
    for (rel, label) in [
        ("VirtualBox VMs", "VirtualBox"),
        (".local/share/gnome-boxes", "GNOME Boxes"),
        (".local/share/libvirt/images", "libvirt"),
        ("Library/Containers/com.utmapp.UTM", "UTM"),
        ("Parallels", "Parallels"),
        (".lima", "Lima"),
        (".vagrant.d/boxes", "Vagrant"),
        ("Emulation", "Emulation"),
    ] {
        add(
            home.join(rel),
            Category::VirtualMachines,
            label,
            Risk::App,
            Action::UseApp { app: label.into() },
            vm_hint.clone(),
            None,
        );
    }

    // Toolchains: old versions pile up silently.
    for (rel, label, command, hint) in [
        (
            ".rustup/toolchains",
            "rustup",
            "rustup toolchain list",
            tr(
                lang,
                "Remova toolchains antigas com `rustup toolchain uninstall <versão>`.",
                "Remove old toolchains with `rustup toolchain uninstall <version>`.",
            ),
        ),
        (
            ".local/share/mise/installs",
            "mise",
            "mise prune",
            tr(
                lang,
                "`mise prune` remove versões que nenhum projeto usa.",
                "`mise prune` removes versions no project uses.",
            ),
        ),
        (
            ".nvm/versions",
            "nvm",
            "nvm ls",
            tr(
                lang,
                "Remova versões antigas do Node com `nvm uninstall <versão>`.",
                "Remove old Node versions with `nvm uninstall <version>`.",
            ),
        ),
        (
            ".pyenv/versions",
            "pyenv",
            "pyenv versions",
            tr(
                lang,
                "Remova versões antigas do Python com `pyenv uninstall <versão>`.",
                "Remove old Python versions with `pyenv uninstall <version>`.",
            ),
        ),
        (
            "fvm/versions",
            "FVM",
            "fvm list",
            tr(
                lang,
                "Remova SDKs Flutter antigos com `fvm remove <versão>`.",
                "Remove old Flutter SDKs with `fvm remove <version>`.",
            ),
        ),
        (
            ".sdkman/candidates",
            "SDKMAN",
            "sdk list",
            tr(
                lang,
                "Remova JDKs/SDKs antigos com `sdk uninstall <candidato> <versão>`.",
                "Remove old JDKs/SDKs with `sdk uninstall <candidate> <version>`.",
            ),
        ),
        (
            "Android/Sdk",
            "Android SDK",
            "sdkmanager --list_installed",
            tr(
                lang,
                "Remova system images e build-tools antigos pelo SDK Manager do Android Studio.",
                "Remove old system images and build-tools from Android Studio's SDK Manager.",
            ),
        ),
    ] {
        add(
            home.join(rel),
            Category::Toolchains,
            label,
            Risk::Review,
            Action::Command {
                command: command.into(),
            },
            hint.to_string(),
            None,
        );
    }

    // Trash.
    for rel in [".local/share/Trash", ".Trash"] {
        add(
            home.join(rel),
            Category::Trash,
            tr(lang, "Lixeira", "Trash"),
            Risk::Safe,
            Action::Review,
            tr(
                lang,
                "Esvazie a lixeira pelo gerenciador de arquivos.",
                "Empty the trash from your file manager.",
            )
            .to_string(),
            None,
        );
    }

    out
}

/// Directories that only group other folders; their children are reported,
/// never the container itself.
fn container_dirs(known: &KnownDirs, roots: &[PathBuf]) -> HashSet<PathBuf> {
    let home = &known.home;
    let mut set: HashSet<PathBuf> = roots.iter().cloned().collect();
    set.insert(home.clone());
    for rel in [
        ".local",
        ".local/share",
        ".local/state",
        ".config",
        ".cache",
        ".var",
        ".var/app",
        "Library",
        "Library/Application Support",
        "Library/Caches",
        "Library/Containers",
        "Library/Group Containers",
    ] {
        set.insert(home.join(rel));
    }
    if let Some(temp) = &known.temp {
        set.insert(temp.clone());
    }
    set
}

fn target_category(target: &KnownTarget) -> Category {
    let name = target.name.to_lowercase();
    if name.contains("docker") || name.contains("podman") {
        Category::Containers
    } else if name.contains("avd")
        || name.contains("emulator")
        || name.contains("simulator")
        || name.contains("system image")
    {
        Category::VirtualMachines
    } else if name.contains("mise")
        || name.contains("toolchain")
        || name.contains("sdk")
        || name.contains("xcode")
    {
        Category::Toolchains
    } else if name.contains("trash") {
        Category::Trash
    } else {
        Category::DevCaches
    }
}

fn idle_days(stat: &DirStat, now: u64) -> Option<u64> {
    (stat.newest > 0 && now >= stat.newest).then(|| (now - stat.newest) / SECS_PER_DAY)
}

fn is_within(path: &Path, ancestor: &Path) -> bool {
    path.starts_with(ancestor)
}

/// Index of direct children (only directories kept in `dirs`).
fn children_index(dirs: &HashMap<PathBuf, DirStat>) -> HashMap<&Path, Vec<(&Path, u64)>> {
    let mut index: HashMap<&Path, Vec<(&Path, u64)>> = HashMap::new();
    for (path, stat) in dirs {
        if let Some(parent) = path.parent() {
            index
                .entry(parent)
                .or_default()
                .push((path.as_path(), stat.bytes));
        }
    }
    for children in index.values_mut() {
        children.sort_by_key(|&(_, bytes)| std::cmp::Reverse(bytes));
    }
    index
}

fn top_children(
    index: &HashMap<&Path, Vec<(&Path, u64)>>,
    dir: &Path,
    skip: &dyn Fn(&Path) -> bool,
    limit: usize,
    min_bytes: u64,
) -> Vec<Detail> {
    index
        .get(dir)
        .map(|children| {
            children
                .iter()
                .filter(|(path, bytes)| *bytes >= min_bytes && !skip(path))
                .take(limit)
                .map(|(path, bytes)| Detail {
                    name: path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    ),
                    bytes: *bytes,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Present a path relative to the home directory (`~/...`).
pub fn display_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

fn big_file_kind(path: &Path, lang: Language) -> (&'static str, Risk, String) {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "iso" | "img" | "dmg" | "qcow2" | "vdi" | "vmdk" | "vhd" | "vhdx" => (
            tr(lang, "imagem de disco", "disk image"),
            Risk::Review,
            tr(
                lang,
                "Imagens de disco e ISOs já usadas podem ser baixadas de novo.",
                "Disk images and ISOs you already used can be downloaded again.",
            )
            .to_string(),
        ),
        "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "xz" | "bz2" => (
            tr(lang, "arquivo compactado", "archive"),
            Risk::Review,
            tr(
                lang,
                "Se já extraiu, o arquivo compactado provavelmente pode ir embora.",
                "If you already extracted it, the archive can probably go.",
            )
            .to_string(),
        ),
        "mkv" | "mp4" | "mov" | "avi" | "webm" | "m4v" => (
            tr(lang, "vídeo", "video"),
            Risk::Review,
            tr(
                lang,
                "Vídeos grandes fora da pasta de vídeos: mova ou apague.",
                "Large videos outside your videos folder: move or delete them.",
            )
            .to_string(),
        ),
        "db" | "sqlite" | "sqlite3" => (
            tr(lang, "banco SQLite", "SQLite database"),
            Risk::Review,
            tr(
                lang,
                "Banco inchado de algum app. Com o app fechado, `sqlite3 <arquivo> 'VACUUM;'` costuma encolher bastante; apagar zera o histórico do app.",
                "Bloated app database. With the app closed, `sqlite3 <file> 'VACUUM;'` often shrinks it a lot; deleting it wipes the app's history.",
            )
            .to_string(),
        ),
        "log" => (
            tr(lang, "log", "log"),
            Risk::Review,
            tr(
                lang,
                "Logs gigantes raramente são úteis: trunque ou apague.",
                "Huge logs are rarely useful: truncate or delete them.",
            )
            .to_string(),
        ),
        _ => (
            tr(lang, "arquivo grande", "large file"),
            Risk::Review,
            tr(
                lang,
                "Confira de qual app é antes de apagar.",
                "Check which app owns it before deleting.",
            )
            .to_string(),
        ),
    }
}

/// Classify the explored data into findings, largest first.
pub fn build_findings(
    data: &ExploreData,
    known: &KnownDirs,
    targets: &[KnownTarget],
    opts: &InsightOptions,
    lang: Language,
) -> Vec<Finding> {
    let home = &known.home;
    let index = children_index(&data.dirs);
    let detail_min = (opts.min_bytes / 10).max(50_000_000);
    let mut findings: Vec<Finding> = Vec::new();
    let mut claimed: HashSet<PathBuf> = HashSet::new();

    // 1. Targets acari already knows how to clean.
    for target in targets {
        let Some(stat) = data.dirs.get(&target.path) else {
            continue;
        };
        if stat.bytes < opts.min_bytes / 4 || !claimed.insert(target.path.clone()) {
            continue;
        }
        findings.push(Finding {
            path: target.path.clone(),
            bytes: stat.bytes,
            files: stat.files,
            category: target_category(target),
            label: target.name.clone(),
            risk: if target.dangerous {
                Risk::Review
            } else {
                Risk::Safe
            },
            action: Action::AcariTarget {
                target: target.name.clone(),
            },
            hint: target.description.clone(),
            idle_days: idle_days(stat, opts.now),
            details: Vec::new(),
        });
    }

    // 2. Well-known locations (games, media, containers, VMs, toolchains...).
    for loc in known_locations(known, lang) {
        let Some(stat) = data.dirs.get(&loc.path) else {
            continue;
        };
        if stat.bytes < opts.min_bytes || claimed.iter().any(|c| is_within(&loc.path, c)) {
            continue;
        }
        claimed.insert(loc.path.clone());
        let details_dir = loc
            .details_from
            .as_ref()
            .filter(|dir| data.dirs.contains_key(*dir))
            .unwrap_or(&loc.path);
        let claimed_ref = &claimed;
        let details = top_children(
            &index,
            details_dir,
            &|p| claimed_ref.contains(p),
            5,
            detail_min,
        );
        findings.push(Finding {
            path: loc.path,
            bytes: stat.bytes,
            files: stat.files,
            category: loc.category,
            label: loc.label,
            risk: loc.risk,
            action: loc.action,
            hint: loc.hint,
            idle_days: idle_days(stat, opts.now),
            details,
        });
    }

    // 3. Project build outputs (outermost only: node_modules inside
    //    node_modules is already counted by its parent).
    let mut builds: Vec<&BuildDir> = data.build_dirs.iter().collect();
    builds.sort_by_key(|b| b.path.components().count());
    let mut taken_builds: Vec<&Path> = Vec::new();
    for build in builds {
        if taken_builds.iter().any(|t| is_within(&build.path, t)) {
            continue;
        }
        let Some(stat) = data.dirs.get(&build.path) else {
            continue;
        };
        taken_builds.push(&build.path);
        if stat.bytes < opts.min_build_bytes {
            continue;
        }
        let project = build.path.parent().unwrap_or(&build.path);
        let idle = idle_days(stat, opts.now);
        let stale = idle.is_some_and(|d| d >= opts.stale_days);
        claimed.insert(build.path.clone());
        findings.push(Finding {
            path: build.path.clone(),
            bytes: stat.bytes,
            files: stat.files,
            category: Category::ProjectBuilds,
            label: display_path(project, home),
            risk: Risk::Safe,
            action: Action::Command {
                command: build.kind.clean_command(&build.path),
            },
            hint: if stale {
                tr(
                    lang,
                    "Build parado há tempo: apagar só custa um rebuild quando voltar ao projeto.",
                    "Stale build: deleting it only costs a rebuild when you return to the project.",
                )
                .to_string()
            } else {
                tr(
                    lang,
                    "Build recente: apague só se precisar do espaço agora (o próximo build sai do zero).",
                    "Recent build: delete only if you need the space now (next build starts from scratch).",
                )
                .to_string()
            },
            idle_days: idle,
            details: Vec::new(),
        });
    }

    // 4. Linked git worktrees.
    for worktree in &data.worktrees {
        let tree = &worktree.path;
        let Some(stat) = data.dirs.get(tree) else {
            continue;
        };
        if stat.bytes < opts.min_bytes / 2 || claimed.contains(tree) {
            continue;
        }
        claimed.insert(tree.clone());
        findings.push(Finding {
            path: tree.clone(),
            bytes: stat.bytes,
            files: stat.files,
            category: Category::Worktrees,
            label: display_path(tree, home),
            risk: Risk::Review,
            action: Action::Command {
                command: match &worktree.main_repo {
                    Some(repo) => format!(
                        "git -C {} worktree remove {}",
                        shell_quote(repo),
                        shell_quote(tree)
                    ),
                    None => format!("git -C {} status", shell_quote(tree)),
                },
            },
            hint: tr(
                lang,
                "Worktree git: confira se não há mudanças sem commit/push antes de remover.",
                "Git worktree: make sure nothing is left uncommitted/unpushed before removing it.",
            )
            .to_string(),
            idle_days: idle_days(stat, opts.now),
            details: Vec::new(),
        });
    }

    // 5. Large single files nobody else explains.
    let managed: Vec<PathBuf> = findings
        .iter()
        .filter(|f| {
            !matches!(
                f.category,
                Category::Downloads | Category::Media | Category::Trash
            )
        })
        .map(|f| f.path.clone())
        .collect();
    let in_media: Vec<PathBuf> = findings
        .iter()
        .filter(|f| matches!(f.category, Category::Downloads | Category::Media))
        .map(|f| f.path.clone())
        .collect();
    for file in &data.big_files {
        if managed.iter().any(|m| is_within(&file.path, m))
            || in_media.iter().any(|m| is_within(&file.path, m))
        {
            continue;
        }
        let (kind, risk, hint) = big_file_kind(&file.path, lang);
        claimed.insert(file.path.clone());
        findings.push(Finding {
            path: file.path.clone(),
            bytes: file.bytes,
            files: 1,
            category: Category::LargeFiles,
            label: format!("{} ({kind})", display_path(&file.path, home)),
            risk,
            action: Action::Review,
            hint,
            idle_days: (file.modified > 0 && opts.now >= file.modified)
                .then(|| (opts.now - file.modified) / SECS_PER_DAY),
            details: Vec::new(),
        });
    }

    // 6. Whatever is still unexplained: big folders directly below the
    //    containers (home, ~/.local/share, ~/.config, /tmp...).
    let containers = container_dirs(known, &data.roots);
    let explained: Vec<(PathBuf, u64)> =
        findings.iter().map(|f| (f.path.clone(), f.bytes)).collect();
    let mut candidates: Vec<(&PathBuf, &DirStat)> = data
        .dirs
        .iter()
        .filter(|(path, _)| {
            !containers.contains(*path)
                && path.parent().is_some_and(|p| containers.contains(p))
                && !explained.iter().any(|(e, _)| is_within(path, e))
        })
        .collect();
    candidates.sort_by_key(|(_, stat)| std::cmp::Reverse(stat.bytes));
    let temp = known.temp.as_deref();
    for (path, stat) in candidates {
        let inner = outermost_sum(
            explained
                .iter()
                .filter(|(e, _)| is_within(e, path) && e != path),
        );
        let remainder = stat.bytes.saturating_sub(inner);
        if remainder < opts.min_bytes {
            continue;
        }
        let is_temp = temp.is_some_and(|t| path.parent() == Some(t));
        let claimed_ref = &claimed;
        let details = top_children(
            &index,
            path,
            &|p| {
                claimed_ref
                    .iter()
                    .any(|c| is_within(c, p) || is_within(p, c))
            },
            5,
            detail_min,
        );
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let parent = path.parent().unwrap_or(path);
        let is_app_data = [
            ".local/share",
            ".config",
            ".var/app",
            "Library/Application Support",
            "Library/Containers",
        ]
        .iter()
        .any(|rel| parent == home.join(rel));
        let hint = if is_temp {
            tr(
                lang,
                "Temporários de algum programa: com ele fechado, normalmente pode apagar.",
                "Some program's temporary files: with it closed, usually safe to delete.",
            )
            .to_string()
        } else if is_app_data {
            match lang {
                Language::Portuguese => format!(
                    "Dados do app \"{name}\". Veja nas configurações do app se dá para limpar histórico/cache por lá."
                ),
                Language::English => format!(
                    "Data of the \"{name}\" app. Check its settings for a way to clear history/cache."
                ),
            }
        } else {
            tr(
                lang,
                "Pasta grande sem regra conhecida: abra os maiores itens abaixo e decida.",
                "Large folder with no known rule: open the biggest items below and decide.",
            )
            .to_string()
        };
        findings.push(Finding {
            path: path.clone(),
            bytes: remainder,
            files: stat.files,
            category: if is_temp {
                Category::Temp
            } else {
                Category::Other
            },
            label: display_path(path, home),
            risk: Risk::Review,
            action: Action::Review,
            hint,
            idle_days: idle_days(stat, opts.now),
            details,
        });
    }

    findings.sort_by_key(|f| std::cmp::Reverse(f.bytes));
    findings
}

/// Sum bytes counting nested paths once (outermost wins).
fn outermost_sum<'a>(items: impl Iterator<Item = &'a (PathBuf, u64)>) -> u64 {
    let mut items: Vec<&(PathBuf, u64)> = items.collect();
    items.sort_by_key(|(p, _)| p.components().count());
    let mut kept: Vec<&Path> = Vec::new();
    let mut total = 0_u64;
    for (path, bytes) in items {
        if kept.iter().any(|k| is_within(path, k)) {
            continue;
        }
        kept.push(path);
        total = total.saturating_add(*bytes);
    }
    total
}

/// Total of findings counting nested paths once. `Other`/`Temp` findings
/// already exclude what is explained inside them, so they always add up.
pub fn non_overlapping_total<'a>(findings: impl IntoIterator<Item = &'a Finding>) -> u64 {
    let mut residual = 0_u64;
    let mut specific: Vec<(PathBuf, u64)> = Vec::new();
    for finding in findings {
        if matches!(finding.category, Category::Other | Category::Temp) {
            residual = residual.saturating_add(finding.bytes);
        } else {
            specific.push((finding.path.clone(), finding.bytes));
        }
    }
    residual.saturating_add(outermost_sum(specific.iter()))
}

/// Quote a path for a POSIX shell when it needs quoting.
pub fn shell_quote(path: &Path) -> String {
    shell_quote_str(&path.display().to_string())
}

fn shell_quote_str(raw: &str) -> String {
    let safe = !raw.is_empty()
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+=:@~,".contains(c));
    if safe {
        raw.to_string()
    } else {
        format!("'{}'", raw.replace('\'', "'\\''"))
    }
}

fn tr(lang: Language, pt: &'static str, en: &'static str) -> &'static str {
    match lang {
        Language::Portuguese => pt,
        Language::English => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;
    const DAY: u64 = SECS_PER_DAY;
    const NOW: u64 = 1_000 * DAY;

    fn stat(bytes: u64, newest: u64) -> DirStat {
        DirStat {
            bytes,
            files: 1,
            newest,
        }
    }

    fn fixture() -> (ExploreData, KnownDirs) {
        let home = PathBuf::from("/home/u");
        let mut dirs = HashMap::new();
        let mut put = |rel: &str, bytes: u64| {
            dirs.insert(
                if rel.is_empty() {
                    home.clone()
                } else {
                    home.join(rel)
                },
                stat(bytes, NOW - 60 * DAY),
            );
        };
        put("", 400 * GB);
        put(".local", 150 * GB);
        put(".local/share", 150 * GB);
        put(".local/share/Steam", 100 * GB);
        put(".local/share/Steam/steamapps", 100 * GB);
        put(".local/share/Steam/steamapps/common", 100 * GB);
        put(".local/share/Steam/steamapps/common/BigGame", 70 * GB);
        put(".local/share/Steam/steamapps/common/SmallGame", 30 * GB);
        put(".local/share/someapp", 30 * GB);
        put(".local/share/tinyapp", GB / 10);
        put("Videos", 100 * GB);
        put("Videos/obs", 99 * GB);
        put("projects", 120 * GB);
        put("projects/a", 100 * GB);
        put("projects/a/target", 90 * GB);
        put("projects/b", 15 * GB);
        put("projects/b/node_modules", 10 * GB);
        put("projects/b/node_modules/x/node_modules", 2 * GB);
        put(".cache", 30 * GB);
        put(".cache/go-build", 30 * GB);
        let data = ExploreData {
            roots: vec![home.clone()],
            dirs,
            big_files: vec![
                BigFile {
                    path: home.join(".local/share/someapp/app.db"),
                    bytes: 25 * GB,
                    modified: NOW - DAY,
                },
                BigFile {
                    path: home.join("Videos/obs/rec.mkv"),
                    bytes: 5 * GB,
                    modified: NOW - DAY,
                },
            ],
            build_dirs: vec![
                BuildDir {
                    path: home.join("projects/a/target"),
                    kind: BuildKind::Cargo,
                },
                BuildDir {
                    path: home.join("projects/b/node_modules"),
                    kind: BuildKind::Node,
                },
                BuildDir {
                    path: home.join("projects/b/node_modules/x/node_modules"),
                    kind: BuildKind::Node,
                },
            ],
            worktrees: Vec::new(),
            errors: 0,
        };
        let known = KnownDirs {
            home: home.clone(),
            videos: Some(home.join("Videos")),
            ..KnownDirs::default()
        };
        (data, known)
    }

    fn opts() -> InsightOptions {
        InsightOptions {
            now: NOW,
            ..InsightOptions::default()
        }
    }

    fn targets() -> Vec<KnownTarget> {
        vec![KnownTarget {
            name: "Go Build Cache".into(),
            path: PathBuf::from("/home/u/.cache/go-build"),
            description: "go build cache".into(),
            dangerous: false,
        }]
    }

    fn find<'a>(findings: &'a [Finding], rel: &str) -> &'a Finding {
        let path = PathBuf::from("/home/u").join(rel);
        findings
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("no finding for {rel}: {findings:#?}"))
    }

    #[test]
    fn classifies_known_locations_targets_and_builds() {
        let (data, known) = fixture();
        let findings = build_findings(&data, &known, &targets(), &opts(), Language::English);

        let steam = find(&findings, ".local/share/Steam");
        assert_eq!(steam.category, Category::Games);
        assert_eq!(steam.risk, Risk::App);
        assert_eq!(steam.details[0].name, "BigGame");

        let go = find(&findings, ".cache/go-build");
        assert_eq!(go.category, Category::DevCaches);
        assert_eq!(
            go.action,
            Action::AcariTarget {
                target: "Go Build Cache".into()
            }
        );

        let cargo = find(&findings, "projects/a/target");
        assert_eq!(cargo.category, Category::ProjectBuilds);
        assert_eq!(cargo.idle_days, Some(60));
        assert!(
            cargo
                .action
                .command_line()
                .unwrap()
                .starts_with("cargo clean --manifest-path")
        );

        assert!(
            !findings.iter().any(|f| f.path.ends_with("x/node_modules")),
            "nested build dirs are counted by their outermost parent"
        );
        assert_eq!(find(&findings, "Videos").category, Category::Media);
    }

    #[test]
    fn surfaces_unexplained_app_data_and_bloated_files() {
        let (data, known) = fixture();
        let findings = build_findings(&data, &known, &targets(), &opts(), Language::English);

        let db = find(&findings, ".local/share/someapp/app.db");
        assert_eq!(db.category, Category::LargeFiles);
        assert!(db.label.contains("SQLite"));

        // someapp is 30 GB, 25 GB of which is explained by the database.
        let app = find(&findings, ".local/share/someapp");
        assert_eq!(app.category, Category::Other);
        assert_eq!(app.bytes, 5 * GB);

        // Files inside media folders are covered by the media finding.
        assert!(!findings.iter().any(|f| f.path.ends_with("rec.mkv")));
        // Small folders stay out.
        assert!(!findings.iter().any(|f| f.path.ends_with("tinyapp")));
    }

    #[test]
    fn leftover_folder_reports_only_unexplained_bytes() {
        let (data, known) = fixture();
        let findings = build_findings(&data, &known, &targets(), &opts(), Language::English);
        // projects = 120 GB; 90 + 10 GB are builds.
        let projects = find(&findings, "projects");
        assert_eq!(projects.bytes, 20 * GB);
        assert_eq!(projects.category, Category::Other);
    }

    #[test]
    fn totals_count_nested_findings_once() {
        let (data, known) = fixture();
        let findings = build_findings(&data, &known, &targets(), &opts(), Language::English);
        let total = non_overlapping_total(&findings);
        // Everything in the fixture home is explained: 400 GB minus the
        // 0.1 GB tinyapp and the 20 GB of .local/share not in any child.
        assert!(total <= 400 * GB, "total {total} exceeds home size");
        assert!(total >= 370 * GB, "total {total} misses findings");
    }

    #[test]
    fn detects_build_dirs_only_with_markers() {
        assert_eq!(
            BuildKind::detect("target", |m| m == "Cargo.toml"),
            Some(BuildKind::Cargo)
        );
        assert_eq!(BuildKind::detect("target", |_| false), None);
        assert_eq!(
            BuildKind::detect("build", |m| m == "pubspec.yaml"),
            Some(BuildKind::Dart)
        );
        assert_eq!(BuildKind::detect("src", |_| true), None);
    }

    #[test]
    fn quotes_paths_for_the_shell() {
        assert_eq!(shell_quote(Path::new("/a/b-c.d")), "/a/b-c.d");
        assert_eq!(shell_quote(Path::new("/a/b c")), "'/a/b c'");
        assert_eq!(shell_quote(Path::new("/a/it's")), "'/a/it'\\''s'");
    }

    #[test]
    fn displays_paths_relative_to_home() {
        let home = Path::new("/home/u");
        assert_eq!(display_path(Path::new("/home/u"), home), "~");
        assert_eq!(display_path(Path::new("/home/u/x"), home), "~/x");
        assert_eq!(display_path(Path::new("/tmp/x"), home), "/tmp/x");
    }
}
