use clap::Parser;

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
}
