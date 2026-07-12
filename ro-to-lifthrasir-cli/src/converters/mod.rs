pub mod accessory;
pub mod item;
pub mod job;
pub mod skill;
pub mod status_icon;
pub mod weapon;

use crate::grf_vfs::GrfVfs;
use anyhow::Context;
use std::path::Path;

/// English item/job names live in the on-disk SystemEN translation project
/// (zackdreaver/llchrisll), not the GRF. These are plaintext Lua, so no
/// decompile step is needed.
const SYSTEM_EN_DIR: &str = "assets/SystemEN";

pub(crate) fn read_system_en(rel: &str) -> anyhow::Result<Vec<u8>> {
    let path = Path::new(SYSTEM_EN_DIR).join(rel);
    std::fs::read(&path).with_context(|| format!("reading SystemEN file: {}", path.display()))
}

type ConverterFn = fn(&GrfVfs, &Path) -> anyhow::Result<()>;

const CONVERTERS: &[(&str, ConverterFn)] = &[
    ("job", job::run),
    ("item", item::run),
    ("skill", skill::run),
    ("accessory", accessory::run),
    ("weapon", weapon::run),
    ("status_icon", status_icon::run),
];

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
