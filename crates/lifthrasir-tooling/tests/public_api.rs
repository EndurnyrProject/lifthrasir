use lifthrasir_tooling::*;
use std::path::Path;

#[test]
fn public_facade_and_asset_reader_need_no_runtime() {
    let source = GrfVfs::open(&[]).unwrap();
    let read: &dyn AssetRead = &source;
    assert!(read.read_asset("data/model/missing.rsm").is_none());
    let _: fn(Option<&str>, &GrfVfs, &Path) -> anyhow::Result<OperationReport> = convert_catalogs;
    let _: fn(&GrfVfs, &str, &Path, &Path, &Path, bool) -> anyhow::Result<OperationReport> =
        convert_map;
    let _: fn(&GrfVfs, &Path, &Path, &Path, bool) -> anyhow::Result<OperationReport> = convert_maps;
    let _: fn(&GrfVfs, &Path, &[String], bool) -> anyhow::Result<OperationReport> = convert_props;
    let _: fn(&GrfVfs, &Path, Option<&str>) -> anyhow::Result<OperationReport> = convert_gr2;
    let _: fn(&GrfVfs, &Path) -> anyhow::Result<usize> = extract_corpus;
    let _: fn(&GrfVfs, &Path, &Path) -> anyhow::Result<PreflightReport> = write_preflight;
    let _: fn(&GrfVfs, &Path, &Path, &Path, bool) -> anyhow::Result<CorpusReport> = convert_corpus;
}
