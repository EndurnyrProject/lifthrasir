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
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "assets/data/ron")]
        out: PathBuf,
        #[arg(long)]
        only: Option<String>,
    },
    ConvertMap {
        #[arg(long)]
        map: String,
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "assets/data/maps")]
        out: PathBuf,
        #[arg(long, default_value = "assets/data/models")]
        models_dir: PathBuf,
        #[arg(long)]
        force_models: bool,
    },
    ConvertMaps {
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "assets/data/maps")]
        out: PathBuf,
        #[arg(long, default_value = "assets/data/models")]
        models_dir: PathBuf,
        #[arg(long)]
        force_models: bool,
    },
    ModelCorpus {
        #[command(subcommand)]
        action: ModelCorpusCommand,
    },
    GenProto {
        #[arg(long)]
        src: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum ModelCorpusCommand {
    Extract {
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "target/rsm2-corpus/extracted")]
        out: PathBuf,
    },
    Scan {
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "target/rsm2-corpus/extracted")]
        extracted: PathBuf,
        #[arg(long, default_value = "target/rsm2-corpus/preflight-report.json")]
        report: PathBuf,
    },
    Convert {
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "target/rsm2-corpus/extracted")]
        extracted: PathBuf,
        #[arg(long, default_value = "target/rsm2-corpus/converted")]
        out: PathBuf,
        #[arg(long, default_value = "target/rsm2-corpus/report.json")]
        report: PathBuf,
        #[arg(long)]
        force: bool,
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
        Command::ConvertMap {
            map,
            loader,
            out,
            models_dir,
            force_models,
        } => {
            let config = config::LoaderConfig::from_path(&loader)?;
            let grfs = config.grfs_by_priority();
            let vfs = grf_vfs::GrfVfs::open(&grfs)?;
            converters::map::run(&vfs, &map, &out, &models_dir, force_models)?;
        }
        Command::ConvertMaps {
            loader,
            out,
            models_dir,
            force_models,
        } => {
            let config = config::LoaderConfig::from_path(&loader)?;
            let grfs = config.grfs_by_priority();
            let vfs = grf_vfs::GrfVfs::open(&grfs)?;
            converters::maps::run(&vfs, &out, &models_dir, force_models)?;
        }
        Command::ModelCorpus { action } => match action {
            ModelCorpusCommand::Extract { loader, out } => {
                let config = config::LoaderConfig::from_path(&loader)?;
                let grfs = config.grfs_by_priority();
                let vfs = grf_vfs::GrfVfs::open(&grfs)?;
                let count = converters::model::corpus::extract(&vfs, &out)?;
                println!("extracted {count} corpus files to {}", out.display());
            }
            ModelCorpusCommand::Scan {
                loader,
                extracted,
                report,
            } => {
                let config = config::LoaderConfig::from_path(&loader)?;
                let grfs = config.grfs_by_priority();
                let vfs = grf_vfs::GrfVfs::open(&grfs)?;
                let preflight =
                    converters::model::corpus::write_preflight(&vfs, &report, &extracted)?;
                println!(
                    "models: {} physical / {} effective, placements: {}, errors: {}",
                    preflight.summary.physical_models,
                    preflight.summary.effective_models,
                    preflight.summary.placements,
                    preflight.summary.inventory_errors,
                );
                anyhow::ensure!(
                    !preflight.has_gates(),
                    "RSM2 corpus preflight gates: {}; blockers: {}; full report: {}",
                    preflight.gate_message(),
                    preflight.blocking_paths(10).join(", "),
                    report.display()
                );
            }
            ModelCorpusCommand::Convert {
                loader,
                extracted,
                out,
                report,
                force,
            } => {
                let config = config::LoaderConfig::from_path(&loader)?;
                let grfs = config.grfs_by_priority();
                let vfs = grf_vfs::GrfVfs::open(&grfs)?;
                let corpus = converters::model::corpus::convert_corpus(
                    &vfs, &extracted, &out, &report, force,
                )?;
                println!(
                    "models: {}, supported RSM2: {} ({} well-formed), placements: {}, blockers: {}",
                    corpus.totals.physical_models,
                    corpus.totals.supported_rsm2,
                    corpus.totals.well_formed_rsm2,
                    corpus.totals.placements,
                    corpus.blockers.len(),
                );
                anyhow::ensure!(
                    !corpus.has_blockers(),
                    "RSM corpus conversion blocked by: {}; full report: {}",
                    corpus.blocking_paths(10).join(", "),
                    report.display()
                );
            }
        },
        Command::GenProto { src, out } => {
            proto_gen::run(&src, &out)?;
        }
    }
    Ok(())
}
