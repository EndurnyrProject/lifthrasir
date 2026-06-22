mod config;
mod converters;
mod decompile;
mod encoding;
mod grf_vfs;
mod lua;
mod proto_gen;

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
    GenProto {
        #[arg(long)]
        src: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Convert { loader, out, only } => {
            let config = config::LoaderConfig::from_path(&loader)?;
            let grfs = config.grfs_by_priority();
            let vfs = grf_vfs::GrfVfs::open(&grfs)?;
            converters::run(only.as_deref(), &vfs, &out)?;
        }
        Command::GenProto { src, out } => {
            proto_gen::run(&src, &out)?;
        }
    }
    Ok(())
}
