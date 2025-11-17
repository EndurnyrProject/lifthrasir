use std::fs;

#[test]
fn test_decompile_jobidentity() {
    let bytecode = fs::read("../assets/data/luafiles514/lua files/datainfo/jobidentity.lub")
        .expect("Failed to read jobidentity.lub");

    let decompiler = luadec::LuaDecompiler::new();
    match decompiler.decompile(&bytecode) {
        Ok(source) => {
            println!("=== jobidentity.lub decompiled successfully ===");
            println!("First 500 chars:\n{}", &source[..source.len().min(500)]);

            let lua = mlua::Lua::new();
            match lua.load(&source).exec() {
                Ok(_) => println!("✓ Successfully executed in mlua"),
                Err(e) => {
                    println!("✗ Execution error: {}", e);
                    println!(
                        "\nFirst 1000 chars of source:\n{}",
                        &source[..source.len().min(1000)]
                    );
                }
            }
        }
        Err(e) => panic!("Decompilation failed: {}", e),
    }
}

#[test]
fn test_decompile_npcidentity() {
    let bytecode = fs::read("../assets/data/luafiles514/lua files/datainfo/npcidentity.lub")
        .expect("Failed to read npcidentity.lub");

    let decompiler = luadec::LuaDecompiler::new();
    match decompiler.decompile(&bytecode) {
        Ok(source) => {
            println!("=== npcidentity.lub decompiled successfully ===");
            println!("First 500 chars:\n{}", &source[..source.len().min(500)]);

            let lua = mlua::Lua::new();
            match lua.load(&source).exec() {
                Ok(_) => println!("✓ Successfully executed in mlua"),
                Err(e) => {
                    println!("✗ Execution error: {}", e);
                    println!(
                        "\nFirst 1000 chars of source:\n{}",
                        &source[..source.len().min(1000)]
                    );
                }
            }
        }
        Err(e) => panic!("Decompilation failed: {}", e),
    }
}

#[test]
fn test_decompile_jobname() {
    let bytecode = fs::read("../assets/data/luafiles514/lua files/datainfo/jobname.lub")
        .expect("Failed to read jobname.lub");

    let decompiler = luadec::LuaDecompiler::new();
    match decompiler.decompile(&bytecode) {
        Ok(source) => {
            println!("=== jobname.lub decompiled successfully ===");
            println!("First 500 chars:\n{}", &source[..source.len().min(500)]);

            let lua = mlua::Lua::new();
            match lua.load(&source).exec() {
                Ok(_) => println!("✓ Successfully executed in mlua"),
                Err(e) => {
                    println!("✗ Execution error: {}", e);
                    println!(
                        "\nFirst 1000 chars of source:\n{}",
                        &source[..source.len().min(1000)]
                    );
                }
            }
        }
        Err(e) => panic!("Decompilation failed: {}", e),
    }
}

#[test]
fn test_decompile_pcjobname() {
    let bytecode = fs::read("../assets/data/luafiles514/lua files/datainfo/pcjobname.lub")
        .expect("Failed to read pcjobname.lub");

    let decompiler = luadec::LuaDecompiler::new();
    match decompiler.decompile(&bytecode) {
        Ok(source) => {
            println!("=== pcjobname.lub decompiled successfully ===");
            println!("First 500 chars:\n{}", &source[..source.len().min(500)]);

            let lua = mlua::Lua::new();
            match lua.load(&source).exec() {
                Ok(_) => println!("✓ Successfully executed in mlua"),
                Err(e) => {
                    println!("✗ Execution error: {}", e);
                    println!("\nFull source:\n{}", source);
                }
            }
        }
        Err(e) => panic!("Decompilation failed: {}", e),
    }
}
