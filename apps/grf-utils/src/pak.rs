#[cfg(test)]
pub use lifthrasir_tooling::collect_entries;

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_pak::{MANIFEST_ENTRY, PakManifest, write_pak_parallel};
    use lifthrasir_tooling::PackInputs;
    use std::io::Read;
    use std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
    };
    use zip::CompressionMethod;

    fn tmp_path_for(path: &Path) -> PathBuf {
        let mut name = path.as_os_str().to_owned();
        name.push(".tmp");
        PathBuf::from(name)
    }

    #[test]
    fn pack_produces_zip_openable_by_plain_zip_archive() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(data_folder.join("sprite/몸통")).unwrap();
        fs::write(data_folder.join("sprite/x.spr"), b"spr-bytes").unwrap();
        fs::write(data_folder.join("sound.ogg"), b"ogg-bytes").unwrap();
        fs::write(
            data_folder.join("sprite/몸통/무희.spr"),
            b"korean-name-bytes",
        )
        .unwrap();

        let PackInputs {
            entries, skipped, ..
        } = collect_entries(&[], Some(&data_folder), &[]);
        assert_eq!(skipped, 0);

        let out_path = dir.path().join("out.pak");
        write_pak_parallel(&entries, &out_path, 7, None, 1, |_| {}).unwrap();

        let file = fs::File::open(&out_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut spr = archive.by_name("data/sprite/x.spr").unwrap();
        let mut spr_contents = Vec::new();
        spr.read_to_end(&mut spr_contents).unwrap();
        assert_eq!(spr_contents, b"spr-bytes");
        drop(spr);

        let ogg = archive.by_name("data/sound.ogg").unwrap();
        assert_eq!(ogg.compression(), CompressionMethod::Stored);
        drop(ogg);

        let mut korean = archive.by_name("data/sprite/몸통/무희.spr").unwrap();
        let mut korean_contents = Vec::new();
        korean.read_to_end(&mut korean_contents).unwrap();
        assert_eq!(korean_contents, b"korean-name-bytes");
        drop(korean);

        let mut manifest_entry = archive.by_name(MANIFEST_ENTRY).unwrap();
        let mut manifest_contents = String::new();
        manifest_entry
            .read_to_string(&mut manifest_contents)
            .unwrap();
        let manifest: PakManifest = toml::from_str(&manifest_contents).unwrap();
        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.content_version, 7);
    }

    #[test]
    fn pack_excludes_map_and_model_source_formats_from_loose_data() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(&data_folder).unwrap();
        for extension in ["rsm", "gnd", "gat", "rsw", "act", "spr", "png", "ogg"] {
            fs::write(data_folder.join(format!("asset.{extension}")), b"bytes").unwrap();
        }

        let PackInputs {
            entries,
            skipped,
            excluded,
            ..
        } = collect_entries(&[], Some(&data_folder), &[]);
        assert_eq!(skipped, 0);
        assert_eq!(excluded, 4);

        let out_path = dir.path().join("out.pak");
        write_pak_parallel(&entries, &out_path, 1, None, 1, |_| {}).unwrap();
        let archive_entries = read_all_entries(&out_path);
        let names: HashSet<&str> = archive_entries.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            HashSet::from([
                "data/asset.act",
                "data/asset.spr",
                "data/asset.png",
                "data/asset.ogg",
                MANIFEST_ENTRY,
            ])
        );
    }

    fn read_all_entries(path: &Path) -> std::collections::HashMap<String, Vec<u8>> {
        let file = fs::File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = archive.file_names().map(String::from).collect();

        names
            .into_iter()
            .map(|name| {
                let mut entry = archive.by_name(&name).unwrap();
                let mut contents = Vec::new();
                entry.read_to_end(&mut contents).unwrap();
                (name, contents)
            })
            .collect()
    }

    #[test]
    fn parallel_pack_matches_sequential_pack_content() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(&data_folder).unwrap();
        for i in 0..7 {
            fs::write(
                data_folder.join(format!("file{i}.txt")),
                vec![b'a' + i as u8; 64],
            )
            .unwrap();
        }
        fs::write(data_folder.join("sound.ogg"), b"ogg-bytes").unwrap();

        let PackInputs {
            entries, skipped, ..
        } = collect_entries(&[], Some(&data_folder), &[]);
        assert_eq!(skipped, 0);

        let sequential_path = dir.path().join("sequential.pak");
        write_pak_parallel(&entries, &sequential_path, 3, None, 1, |_| {}).unwrap();

        let parallel_path = dir.path().join("parallel.pak");
        write_pak_parallel(&entries, &parallel_path, 3, None, 4, |_| {}).unwrap();

        let sequential_entries = read_all_entries(&sequential_path);
        let parallel_entries = read_all_entries(&parallel_path);

        let sequential_names: HashSet<&String> = sequential_entries.keys().collect();
        let parallel_names: HashSet<&String> = parallel_entries.keys().collect();
        assert_eq!(sequential_names, parallel_names);

        for name in sequential_names {
            if name == MANIFEST_ENTRY {
                continue;
            }
            assert_eq!(
                sequential_entries[name], parallel_entries[name],
                "entry '{name}' differs between sequential and parallel pack"
            );
        }

        let sequential_manifest: PakManifest =
            toml::from_str(std::str::from_utf8(&sequential_entries[MANIFEST_ENTRY]).unwrap())
                .unwrap();
        let parallel_manifest: PakManifest =
            toml::from_str(std::str::from_utf8(&parallel_entries[MANIFEST_ENTRY]).unwrap())
                .unwrap();
        assert_eq!(
            sequential_manifest.format_version,
            parallel_manifest.format_version
        );
        assert_eq!(
            sequential_manifest.content_version,
            parallel_manifest.content_version
        );
    }

    #[test]
    fn parallel_pack_leaves_no_tmp_or_part_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(&data_folder).unwrap();
        for i in 0..5 {
            fs::write(data_folder.join(format!("file{i}.txt")), b"payload").unwrap();
        }

        let PackInputs { entries, .. } = collect_entries(&[], Some(&data_folder), &[]);
        let out_path = dir.path().join("out.pak");

        write_pak_parallel(&entries, &out_path, 1, None, 3, |_| {}).unwrap();

        assert!(out_path.exists());
        assert!(!tmp_path_for(&out_path).exists());

        let leftover_parts: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".part"))
            .collect();
        assert!(
            leftover_parts.is_empty(),
            "leftover part files: {leftover_parts:?}"
        );

        let file = fs::File::open(&out_path).unwrap();
        zip::ZipArchive::new(file).unwrap();
    }
}
