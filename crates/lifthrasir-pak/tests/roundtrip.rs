use lifthrasir_pak::{PakReader, SourceEntries, write_pak_parallel};

#[test]
fn writer_round_trips_aliases_codecs_and_unicode_through_reader() {
    let dir = tempfile::tempdir().unwrap();
    for jobs in [1, 4] {
        let path = dir.path().join(format!("archive-{jobs}.pak"));
        let entries: SourceEntries = vec![
            ("data/몸통/a.spr".into(), b"sprite".to_vec()),
            ("data/alias.spr".into(), b"sprite".to_vec()),
            ("data/sound.ogg".into(), b"sound".to_vec()),
            (
                "data/source.gnd".into(),
                b"generic-writer-keeps-source".to_vec(),
            ),
        ];
        let stats = write_pak_parallel(&entries, &path, 7, None, jobs, |_| {}).unwrap();
        assert_eq!(stats.aliased_entries, 1);
        let reader = PakReader::open(&path).unwrap();
        assert_eq!(reader.read("DATA\\몸통\\A.SPR").unwrap(), b"sprite");
        assert_eq!(reader.read("data/alias.spr").unwrap(), b"sprite");
        assert_eq!(reader.read("data/sound.ogg").unwrap(), b"sound");
        assert_eq!(
            reader.read("data/source.gnd").unwrap(),
            b"generic-writer-keeps-source"
        );
        assert_eq!(reader.list_files().len(), 4);
        assert!(!reader.exists(".lifthrasir/manifest.toml"));
    }
}
