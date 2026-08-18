use crate::converters::model::{self, ConvertOutcome, TexturePool};
use crate::grf_vfs::GrfVfs;
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

pub fn run(vfs: &GrfVfs, models_dir: &Path, models: &[String], force: bool) -> anyhow::Result<()> {
    let models: Vec<_> = if models.is_empty() {
        DEFAULT_MODELS.to_vec()
    } else {
        models.iter().map(String::as_str).collect()
    };
    let pool = TexturePool::new(models_dir);
    let mut converted = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for filename in models {
        match model::convert_model(vfs, filename, models_dir, &pool, force) {
            Ok(ConvertOutcome::Converted) => converted += 1,
            Ok(ConvertOutcome::Skipped) => skipped += 1,
            Ok(ConvertOutcome::UnsupportedVersion) => {
                failed += 1;
                eprintln!("failed to convert prop model '{filename}': unsupported model version");
            }
            Err(error) => {
                failed += 1;
                eprintln!("failed to convert prop model '{filename}': {error:#}");
            }
        }
    }

    println!("props: {converted} converted, {failed} failed, {skipped} skipped");
    anyhow::ensure!(failed == 0, "prop conversion failed for {failed} models");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_MODELS;

    #[test]
    fn default_models_are_rsm_files() {
        assert!(!DEFAULT_MODELS.is_empty());
        assert!(DEFAULT_MODELS.iter().all(|model| model.ends_with(".rsm")));
    }
}
