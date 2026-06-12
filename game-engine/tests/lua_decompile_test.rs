use std::path::PathBuf;

use game_engine::infrastructure::ro_formats::GrfFile;

/// Reads a datainfo .lub file straight from the GRF archive,
/// the same source the engine uses at runtime via the ro:// asset source.
fn read_lub(name: &str) -> Vec<u8> {
    let grf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets/data.grf");
    let grf = GrfFile::from_path(grf_path).expect("Failed to open assets/data.grf");

    grf.get_file(&format!("data\\luafiles514\\lua files\\datainfo\\{name}"))
        .unwrap_or_else(|| panic!("{name} not found in data.grf"))
}

fn decompile_and_execute(name: &str) {
    let bytecode = read_lub(name);

    let decompiler = luadec::LuaDecompiler::new();
    let source = decompiler
        .decompile(&bytecode)
        .unwrap_or_else(|e| panic!("Decompilation of {name} failed: {e}"));

    println!("=== {name} decompiled successfully ===");
    println!("First 500 chars:\n{}", &source[..source.len().min(500)]);

    // Execution is informational only: the decompiled datainfo scripts
    // reference tables defined elsewhere and are not standalone programs.
    let lua = mlua::Lua::new();
    match lua.load(&source).exec() {
        Ok(_) => println!("Successfully executed in mlua"),
        Err(e) => println!("Execution error (not a test failure): {e}"),
    }
}

#[test]
fn test_decompile_jobidentity() {
    decompile_and_execute("jobidentity.lub");
}

#[test]
fn test_decompile_npcidentity() {
    decompile_and_execute("npcidentity.lub");
}

#[test]
fn test_decompile_jobname() {
    decompile_and_execute("jobname.lub");
}

#[test]
fn test_decompile_pcjobname() {
    decompile_and_execute("pcjobname.lub");
}
