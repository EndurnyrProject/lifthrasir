mod config;
mod grf_vfs;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ro-to-lifthrasir-cli")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Convert {
        #[arg(long, default_value = "assets/loader.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "assets/data/ron")]
        out: PathBuf,
        #[arg(long)]
        only: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Convert { loader, out, only: _ } => {
            let config = config::LoaderConfig::from_path(&loader)?;
            for grf in config.grfs_by_priority() {
                println!("grf: {} (priority {})", grf.path, grf.priority);
            }
            println!("out: {}", out.display());
        }
    }
    Ok(())
}
