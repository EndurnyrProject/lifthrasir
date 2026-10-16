mod proto_gen;

use clap::{Parser, Subcommand};
use lifthrasir_tooling::{GrfVfs, LoaderConfig};
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
    ConvertProps {
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "assets/data/models")]
        models_dir: PathBuf,
        #[arg(long)]
        model: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    ConvertGr2 {
        #[arg(long, default_value = "assets/convert.toml")]
        loader: PathBuf,
        #[arg(long, default_value = "assets/data/models/3dmob")]
        out: PathBuf,
        #[arg(long)]
        model: Option<String>,
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

struct CorpusLogger;
impl log::Log for CorpusLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{}", record.args());
        }
    }
    fn flush(&self) {}
}
static CORPUS_LOGGER: CorpusLogger = CorpusLogger;

fn render_report(report: &lifthrasir_tooling::OperationReport) {
    for diagnostic in &report.diagnostics {
        match diagnostic.level {
            lifthrasir_tooling::DiagnosticLevel::Info => println!("{}", diagnostic.message),
            lifthrasir_tooling::DiagnosticLevel::Warning => eprintln!("{}", diagnostic.message),
        }
    }
}

fn present(result: anyhow::Result<lifthrasir_tooling::OperationReport>) -> anyhow::Result<()> {
    match result {
        Ok(report) => {
            render_report(&report);
            Ok(())
        }
        Err(error) => {
            if let Some(batch) = error.downcast_ref::<lifthrasir_tooling::BatchFailure>() {
                render_report(&batch.report);
            }
            Err(error)
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if matches!(&cli.command, Command::ModelCorpus { .. }) {
        log::set_logger(&CORPUS_LOGGER)
            .map_err(|error| anyhow::anyhow!("installing corpus diagnostics logger: {error}"))?;
        log::set_max_level(log::LevelFilter::Info);
    }
    match cli.command {
        Command::Convert { loader, out, only } => {
            let config = LoaderConfig::from_path(&loader)?;
            let grfs = config.grfs_by_priority();
            let vfs = GrfVfs::open(&grfs)?;
            present(lifthrasir_tooling::convert_catalogs(
                only.as_deref(),
                &vfs,
                &out,
            ))?;
        }
        Command::ConvertMap {
            map,
            loader,
            out,
            models_dir,
            force_models,
        } => {
            let config = LoaderConfig::from_path(&loader)?;
            let grfs = config.grfs_by_priority();
            let vfs = GrfVfs::open(&grfs)?;
            present(lifthrasir_tooling::convert_map(
                &vfs,
                &map,
                &out,
                &models_dir,
                force_models,
            ))?;
        }
        Command::ConvertMaps {
            loader,
            out,
            models_dir,
            force_models,
        } => {
            let config = LoaderConfig::from_path(&loader)?;
            let grfs = config.grfs_by_priority();
            let vfs = GrfVfs::open(&grfs)?;
            present(lifthrasir_tooling::convert_maps(
                &vfs,
                &out,
                &models_dir,
                force_models,
            ))?;
        }
        Command::ConvertProps {
            loader,
            models_dir,
            model,
            force,
        } => {
            let config = LoaderConfig::from_path(&loader)?;
            let grfs = config.grfs_by_priority();
            let vfs = GrfVfs::open(&grfs)?;
            present(lifthrasir_tooling::convert_props(
                &vfs,
                &models_dir,
                &model,
                force,
            ))?;
        }
        Command::ConvertGr2 { loader, out, model } => {
            let config = LoaderConfig::from_path(&loader)?;
            let vfs = GrfVfs::open(&config.grfs_by_priority())?;
            present(lifthrasir_tooling::convert_gr2(
                &vfs,
                &out,
                model.as_deref(),
            ))?;
        }
        Command::ModelCorpus { action } => match action {
            ModelCorpusCommand::Extract { loader, out } => {
                let config = LoaderConfig::from_path(&loader)?;
                let grfs = config.grfs_by_priority();
                let vfs = GrfVfs::open(&grfs)?;
                let count = lifthrasir_tooling::extract_corpus(&vfs, &out)?;
                println!("extracted {count} corpus files to {}", out.display());
            }
            ModelCorpusCommand::Scan {
                loader,
                extracted,
                report,
            } => {
                let config = LoaderConfig::from_path(&loader)?;
                let grfs = config.grfs_by_priority();
                let vfs = GrfVfs::open(&grfs)?;
                let preflight = lifthrasir_tooling::write_preflight(&vfs, &report, &extracted)?;
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
                let config = LoaderConfig::from_path(&loader)?;
                let grfs = config.grfs_by_priority();
                let vfs = GrfVfs::open(&grfs)?;
                let corpus =
                    lifthrasir_tooling::convert_corpus(&vfs, &extracted, &out, &report, force)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_names_defaults_and_proto_dispatch_remain_compatible() {
        Cli::command().debug_assert();
        let cli = Cli::try_parse_from(["ro-to-lifthrasir-cli", "convert-map", "--map", "prontera"])
            .unwrap();
        let Command::ConvertMap {
            map,
            loader,
            out,
            models_dir,
            force_models,
        } = cli.command
        else {
            panic!("wrong command");
        };
        assert_eq!(map, "prontera");
        assert_eq!(loader, PathBuf::from("assets/convert.toml"));
        assert_eq!(out, PathBuf::from("assets/data/maps"));
        assert_eq!(models_dir, PathBuf::from("assets/data/models"));
        assert!(!force_models);
        assert!(matches!(
            Cli::try_parse_from([
                "ro-to-lifthrasir-cli",
                "gen-proto",
                "--src",
                "proto",
                "--out",
                "out.rs"
            ])
            .unwrap()
            .command,
            Command::GenProto { .. }
        ));
    }
}
