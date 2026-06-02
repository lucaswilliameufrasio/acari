use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about = "acari: cache scanner for macOS and Linux")]
pub struct Cli {
    /// Limit scan to specific target names (can be used multiple times)
    #[arg(short, long = "target")]
    pub targets: Vec<String>,

    /// Skip the scan and print available targets
    #[arg(long)]
    pub list: bool,

    /// Run without TUI
    #[arg(long)]
    pub headless: bool,

    /// Add an ad-hoc path to scan (can be used multiple times)
    #[arg(long = "scan-path")]
    pub scan_paths: Vec<String>,

    /// In headless mode, run clean immediately after scan
    #[arg(long)]
    pub clean: bool,

    /// Simulate cleaning without deleting files (requires --clean)
    #[arg(long, requires = "clean")]
    pub dry_run: bool,

    /// Confirm destructive cleaning in headless mode
    #[arg(long, requires = "clean")]
    pub yes: bool,

    /// Exclude directories matching these patterns (can be used multiple times)
    #[arg(long = "exclude")]
    pub excludes: Vec<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Manage custom targets
    Target {
        #[command(subcommand)]
        action: TargetAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum TargetAction {
    /// Add a custom target to the config file
    Add {
        /// Target display name
        name: String,
        /// Absolute path to scan
        path: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Remove a custom target from the config file
    Remove {
        /// Target name to remove
        name: String,
    },
    /// List all custom targets
    List,
}

pub mod target_config;
