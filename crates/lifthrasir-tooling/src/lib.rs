//! Offline content conversion, independent of Bevy and client policy.

mod config;
mod converters;
mod decompile;
mod encoding;
mod grf_vfs;
mod lua;
mod pack_inputs;
mod report;
pub use pack_inputs::{collect_entries, collect_entries_with_pruning};
pub use report::{BatchFailure, Diagnostic, DiagnosticLevel, OperationReport, PackInputs};

pub use config::LoaderConfig;
pub use converters::model::corpus::{CorpusReport, PreflightReport};
pub use grf_vfs::{AssetRead, GrfVfs};
use std::path::Path;

pub fn convert_catalogs(
    only: Option<&str>,
    source: &GrfVfs,
    out: &Path,
) -> anyhow::Result<OperationReport> {
    converters::run(only, source, out)
}

pub fn convert_map(
    source: &GrfVfs,
    map_name: &str,
    out_dir: &Path,
    models_dir: &Path,
    force_models: bool,
) -> anyhow::Result<OperationReport> {
    converters::map::run(source, map_name, out_dir, models_dir, force_models)
}

pub fn convert_maps(
    source: &GrfVfs,
    out_dir: &Path,
    models_dir: &Path,
    force_models: bool,
) -> anyhow::Result<OperationReport> {
    converters::maps::run(source, out_dir, models_dir, force_models)
}

pub fn convert_props(
    source: &GrfVfs,
    models_dir: &Path,
    models: &[String],
    force: bool,
) -> anyhow::Result<OperationReport> {
    converters::props::run(source, models_dir, models, force)
}

pub fn convert_gr2(
    source: &GrfVfs,
    out: &Path,
    selected: Option<&str>,
) -> anyhow::Result<OperationReport> {
    converters::gr2::run(source, out, selected)
}

pub fn extract_corpus(source: &GrfVfs, extracted_root: &Path) -> anyhow::Result<usize> {
    converters::model::corpus::extract(source, extracted_root)
}

pub fn write_preflight(
    source: &GrfVfs,
    output: &Path,
    extracted_root: &Path,
) -> anyhow::Result<PreflightReport> {
    converters::model::corpus::write_preflight(source, output, extracted_root)
}

pub fn convert_corpus(
    source: &GrfVfs,
    extracted_root: &Path,
    output_root: &Path,
    report_path: &Path,
    force: bool,
) -> anyhow::Result<CorpusReport> {
    converters::model::corpus::convert_corpus(
        source,
        extracted_root,
        output_root,
        report_path,
        force,
    )
}
