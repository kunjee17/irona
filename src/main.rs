mod app;
mod deleter;
mod scanner;
mod ui;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "irona", about = "Reclaim disk space from build artifacts")]
struct Args {
    /// Root directory to scan (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn main() {
    let args = Args::parse();
    println!("Scanning: {}", args.path.display());
}
