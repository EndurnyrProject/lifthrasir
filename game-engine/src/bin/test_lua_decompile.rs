fn main() {
    println!("Testing Lua decompilation...\n");

    test_all_files();
}

fn test_all_files() {
    println!("=== Testing all files together (correct order) ===\n");

    let job_identity = std::fs::read("../assets/data/luafiles514/lua files/datainfo/jobidentity.lub")
        .expect("Failed to read jobidentity.lub");
    let npc_identity = std::fs::read("../assets/data/luafiles514/lua files/datainfo/npcidentity.lub")
        .expect("Failed to read npcidentity.lub");
    let job_name = std::fs::read("../assets/data/luafiles514/lua files/datainfo/jobname.lub")
        .expect("Failed to read jobname.lub");
    let pc_job_name = std::fs::read("../assets/data/luafiles514/lua files/datainfo/pcjobname.lub")
        .expect("Failed to read pcjobname.lub");

    let mut decompiler = game_engine::infrastructure::lua_scripts::decompiler::LuaDecompiler::new();

    println!("Decompiling all files...");
    let job_identity_src = decompiler.decompile_bytecode(&job_identity).expect("Failed to decompile jobidentity");
    let npc_identity_src = decompiler.decompile_bytecode(&npc_identity).expect("Failed to decompile npcidentity");
    let job_name_src = decompiler.decompile_bytecode(&job_name).expect("Failed to decompile jobname");
    let pc_job_name_src = decompiler.decompile_bytecode(&pc_job_name).expect("Failed to decompile pcjobname");

    println!("Decompilation successful for all files!\n");

    // Save all decompiled files for inspection
    std::fs::write("jobidentity_decompiled.lua", &job_identity_src.source)
        .expect("Failed to write jobidentity");
    std::fs::write("pcjobname_decompiled.lua", &pc_job_name_src.source)
        .expect("Failed to write pcjobname");
    println!("Saved decompiled files");

    println!("Trying to execute with mlua...");

    let lua = mlua::Lua::new();

    println!("Loading jobidentity...");
    match lua.load(&job_identity_src.source).exec() {
        Ok(_) => {
            println!("✓ jobidentity loaded");

            lua.load("JOBID = JTtbl").exec().expect("Failed to alias JOBID");
            println!("  Set JOBID = JTtbl");

            lua.load(r#"
                setmetatable(JOBID, {
                    __index = function(t, k)
                        return -999999
                    end
                })
            "#).exec().expect("Failed to set metatable");
            println!("  Added metatable to handle missing job IDs");
        }
        Err(e) => {
            println!("✗ jobidentity failed: {}", e);
            println!("\nFirst 20 lines:");
            for (i, line) in job_identity_src.source.lines().take(20).enumerate() {
                println!("{:3}: {}", i + 1, line);
            }
            return;
        }
    }

    println!("Loading npcidentity...");
    match lua.load(&npc_identity_src.source).exec() {
        Ok(_) => println!("✓ npcidentity loaded"),
        Err(e) => {
            println!("✗ npcidentity failed: {}", e);
            return;
        }
    }

    println!("Loading jobname...");
    match lua.load(&job_name_src.source).exec() {
        Ok(_) => println!("✓ jobname loaded"),
        Err(e) => {
            println!("✗ jobname failed: {}", e);
            return;
        }
    }

    println!("Loading pcjobname...");
    match lua.load(&pc_job_name_src.source).set_name("pcjobname").exec() {
        Ok(_) => println!("✓ pcjobname loaded"),
        Err(e) => {
            println!("✗ pcjobname failed: {}", e);
            println!("  First 20 lines:");
            for (i, line) in pc_job_name_src.source.lines().take(20).enumerate() {
                println!("{:3}: {}", i + 1, line);
            }
            return;
        }
    }

    println!("\nTrying to extract JobNameTable and PCJobNameTable...");

    let globals = lua.globals();

    if let Ok(job_name_table) = globals.get::<mlua::Table>("JobNameTable") {
        let mut count = 0;
        for pair in job_name_table.pairs::<mlua::Value, mlua::Value>() {
            if pair.is_ok() {
                count += 1;
            }
        }
        println!("✓ JobNameTable extracted: {} entries", count);
    } else {
        println!("✗ JobNameTable not found!");
    }

    if let Ok(pc_job_name_table) = globals.get::<mlua::Table>("PCJobNameTable") {
        let mut count = 0;
        for pair in pc_job_name_table.pairs::<mlua::Value, mlua::Value>() {
            if pair.is_ok() {
                count += 1;
            }
        }
        println!("✓ PCJobNameTable extracted: {} entries", count);
    } else {
        println!("✗ PCJobNameTable not found!");
    }

    println!("\n✅ Decompilation and extraction successful!");
}
