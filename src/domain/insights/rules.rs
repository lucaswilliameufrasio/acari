//! Classification rules: well-known locations (game libraries, media,
//! containers, VMs, toolchains, trash) and large-file kinds.

use std::path::{Path, PathBuf};

use super::{Action, Category, KnownDirs, Risk, tr};
use crate::i18n::Language;

pub(super) struct Location {
    pub(super) path: PathBuf,
    pub(super) category: Category,
    pub(super) label: String,
    pub(super) risk: Risk,
    pub(super) action: Action,
    pub(super) hint: String,
    /// Sub-folder whose children make the most useful breakdown.
    pub(super) details_from: Option<PathBuf>,
}

pub(super) fn known_locations(known: &KnownDirs, lang: Language) -> Vec<Location> {
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

pub(super) fn big_file_kind(path: &Path, lang: Language) -> (&'static str, Risk, String) {
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
