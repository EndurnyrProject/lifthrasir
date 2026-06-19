use anyhow::Context;
use ro_formats::grf::GrfFile;
use std::path::Path;

use crate::config::GrfEntry;

pub(crate) trait GrfReadable {
    fn get(&self, normalized_path: &str) -> Option<Vec<u8>>;
}

impl GrfReadable for GrfFile {
    fn get(&self, p: &str) -> Option<Vec<u8>> {
        self.get_file(p)
    }
}

pub(crate) fn normalize_path(p: &str) -> String {
    p.replace('/', "\\")
}

pub(crate) fn first_hit(sources: &[impl GrfReadable], logical: &str) -> Option<Vec<u8>> {
    let normalized = normalize_path(logical);
    sources.iter().find_map(|s| s.get(&normalized))
}

pub struct GrfVfs {
    grfs: Vec<GrfFile>,
}

impl GrfVfs {
    pub fn open(grfs: &[&GrfEntry]) -> anyhow::Result<Self> {
        let mut files = Vec::with_capacity(grfs.len());
        for entry in grfs {
            let grf_path = Path::new(&entry.path);
            let candidates = [grf_path.to_path_buf(), Path::new("assets").join(grf_path)];
            let resolved = candidates
                .iter()
                .find(|p| p.exists())
                .with_context(|| format!("GRF not found: {}", entry.path))?;
            let grf = GrfFile::from_path(resolved.clone())
                .with_context(|| format!("Failed to open GRF: {}", resolved.display()))?;
            files.push(grf);
        }
        Ok(Self { grfs: files })
    }

    pub fn read(&self, logical_path: &str) -> Option<Vec<u8>> {
        first_hit(&self.grfs, logical_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeGrf(HashMap<String, Vec<u8>>);

    impl GrfReadable for FakeGrf {
        fn get(&self, path: &str) -> Option<Vec<u8>> {
            self.0.get(path).cloned()
        }
    }

    fn fake(entries: &[(&str, &[u8])]) -> FakeGrf {
        FakeGrf(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_vec()))
                .collect(),
        )
    }

    #[test]
    fn normalize_path_replaces_slashes() {
        assert_eq!(
            normalize_path("data/luafiles514/lua files/x.lub"),
            "data\\luafiles514\\lua files\\x.lub"
        );
    }

    #[test]
    fn normalize_path_already_backslash_unchanged() {
        assert_eq!(normalize_path("data\\foo.txt"), "data\\foo.txt");
    }

    #[test]
    fn first_hit_returns_higher_priority_source() {
        let a = fake(&[("data\\shared.txt", b"from_a")]);
        let b = fake(&[("data\\shared.txt", b"from_b")]);
        let result = first_hit(&[a, b], "data/shared.txt");
        assert_eq!(result.as_deref(), Some(b"from_a" as &[u8]));
    }

    #[test]
    fn first_hit_falls_through_to_later_source() {
        let a = fake(&[]);
        let b = fake(&[("data\\only_in_b.txt", b"found")]);
        let result = first_hit(&[a, b], "data/only_in_b.txt");
        assert_eq!(result.as_deref(), Some(b"found" as &[u8]));
    }

    #[test]
    fn first_hit_returns_none_when_not_found() {
        let a = fake(&[]);
        let result = first_hit(&[a], "data/missing.txt");
        assert!(result.is_none());
    }
}
