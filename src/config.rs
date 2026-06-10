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
    /// Discover and clean project junk (node_modules, target, build, etc.)
    /// Defaults to the management TUI if no subcommand given.
    Project {
        /// Open the management TUI by default
        #[command(subcommand)]
        action: Option<ProjectAction>,
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

#[derive(Debug, Subcommand)]
pub enum ProjectAction {
    /// Add a project root
    AddRoot {
        /// Project root path
        path: String,
    },
    /// Remove a project root
    RemoveRoot {
        /// Project root path to remove
        path: String,
    },
    /// List project roots
    ListRoots,
    /// Add a custom junk directory pattern
    AddPattern {
        /// Directory name to find (e.g., .terraform)
        pattern: String,
    },
    /// Remove a custom pattern
    RemovePattern {
        /// Pattern to remove
        pattern: String,
    },
    /// List all patterns (built-in + custom)
    ListPatterns,
    /// Remove all custom patterns
    ClearPatterns,
    /// Scan project roots for junk directories
    Scan {
        /// Project roots to scan (uses config roots if omitted)
        roots: Vec<String>,
        /// Additional junk patterns
        #[arg(short, long = "pattern")]
        patterns: Vec<String>,
        /// Skip built-in patterns
        #[arg(long)]
        no_default_patterns: bool,
        /// Run without TUI
        #[arg(long)]
        headless: bool,
        /// Clean after scan
        #[arg(long)]
        clean: bool,
        /// Simulate cleaning (requires --clean)
        #[arg(long, requires = "clean")]
        dry_run: bool,
        /// Confirm destructive clean (requires --clean)
        #[arg(long, requires = "clean")]
        yes: bool,
        /// Exclude patterns
        #[arg(long = "exclude")]
        excludes: Vec<String>,
    },
}

pub mod target_config;
