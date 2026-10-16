use crate::OperationReport;
use crate::converters::model::{self, ConvertOutcome, TexturePool};
use crate::grf_vfs::AssetRead;
use std::path::Path;

const DEFAULT_MODELS: &[&str] = &[
    "외부소품/트랩01.rsm",
    "외부소품/트랩02.rsm",
    "외부소품/트랩03.rsm",
    "외부소품/트랩03_2.rsm",
    "외부소품/트랩03_3.rsm",
    "외부소품/트랩03_4.rsm",
    "외부소품/트랩03_5.rsm",
    "외부소품/트랩03_6.rsm",
    "외부소품/트랩04.rsm",
    "외부소품/트랩05.rsm",
];

pub fn run(
    vfs: &impl AssetRead,
    models_dir: &Path,
    models: &[String],
    force: bool,
) -> anyhow::Result<OperationReport> {
    let models: Vec<_> = if models.is_empty() {
        DEFAULT_MODELS.to_vec()
    } else {
        models.iter().map(String::as_str).collect()
    };
    let pool = TexturePool::new(models_dir);
    let mut converted = 0;
    let mut failures = Vec::new();
    let mut report = OperationReport::default();
    let mut skipped = 0;

    for filename in models {
        match model::convert_model(vfs, filename, models_dir, &pool, force) {
            Ok(ConvertOutcome::Converted) => {
                converted += 1;
                report
                    .outputs
                    .push(models_dir.join(model::glb_relative_path(filename)));
            }
            Ok(ConvertOutcome::Skipped) => skipped += 1,
            Ok(ConvertOutcome::UnsupportedVersion) => {
                let message =
                    format!("failed to convert prop model '{filename}': unsupported model version");
                report.warning(message.clone());
                failures.push(message);
            }
            Err(error) => {
                let message = format!("failed to convert prop model '{filename}': {error:#}");
                report.warning(message.clone());
                failures.push(message);
            }
        }
    }

    report.diagnostics.extend(pool.diagnostics());
    report.info(format!(
        "props: {converted} converted, {} failed, {skipped} skipped",
        failures.len()
    ));
    report.finish(failures)
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_MODELS;

    #[test]
    fn partial_failure_keeps_a_published_synthetic_model_and_its_warning() {
        use crate::converters::model::fixtures::{FakeVfs, encode_rsm};
        let temp = tempfile::tempdir().unwrap();
        let source =
            FakeVfs::with(&[("data/model/good.rsm", encode_rsm((1, 4), &["missing.bmp"]))]);
        let error = super::run(
            &source,
            temp.path(),
            &["good.rsm".into(), "missing.rsm".into()],
            false,
        )
        .unwrap_err();
        let batch = error.downcast_ref::<crate::BatchFailure>().unwrap();
        assert_eq!(batch.report.outputs, vec![temp.path().join("good.glb")]);
        assert!(batch.report.outputs[0].is_file());
        assert!(
            batch
                .failures
                .iter()
                .any(|failure| failure.contains("missing.rsm"))
        );
        assert!(
            batch
                .report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("missing.bmp"))
        );
        assert!(batch.report.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 converted, 1 failed, 0 skipped")
        }));
    }

    #[test]
    fn default_models_are_rsm_files() {
        assert!(!DEFAULT_MODELS.is_empty());
        assert!(DEFAULT_MODELS.iter().all(|model| model.ends_with(".rsm")));
    }
}
