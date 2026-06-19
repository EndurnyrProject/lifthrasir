pub mod item;
pub mod job;

use crate::grf_vfs::GrfVfs;
use std::path::Path;

type ConverterFn = fn(&GrfVfs, &Path) -> anyhow::Result<()>;

const CONVERTERS: &[(&str, ConverterFn)] = &[("job", job::run), ("item", item::run)];

pub fn run(only: Option<&str>, vfs: &GrfVfs, out: &Path) -> anyhow::Result<()> {
    if let Some(name) = only {
        if !CONVERTERS.iter().any(|(n, _)| *n == name) {
            anyhow::bail!("unknown converter: {name}");
        }
    }
    for (name, f) in CONVERTERS {
        if only.is_none_or(|o| o == *name) {
            f(vfs, out)?;
        }
    }
    Ok(())
}
