use std::fs;

fn main() {
    let job_identity = fs::read("../assets/data/luafiles514/lua files/datainfo/jobidentity.lub")
        .expect("Failed to read jobidentity.lub");

    let mut decompiler = game_engine::infrastructure::lua_scripts::decompiler::LuaDecompiler::new();
    let job_identity_src = decompiler
        .decompile_bytecode(&job_identity)
        .expect("Failed to decompile jobidentity");

    fs::write("../jobidentity_decompiled.lua", &job_identity_src.source)
        .expect("Failed to write file");

    println!("Saved jobidentity_decompiled.lua");
}
