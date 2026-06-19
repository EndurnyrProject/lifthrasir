use mlua::prelude::*;

pub const UNKNOWN_JOB_SENTINEL: i32 = -999_999;

pub fn new_vm() -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.set_hook(
        mlua::HookTriggers::new().every_nth_instruction(200_000),
        |_lua, _debug| {
            Err(LuaError::RuntimeError(
                "Lua script execution budget exceeded (200k instructions)".into(),
            ))
        },
    )?;
    Ok(lua)
}

pub fn install_job_metatables(lua: &Lua) -> mlua::Result<()> {
    lua.load("JOBID = JTtbl\npcJobTbl = JTtbl").exec()?;
    lua.load(format!(
        r#"
        setmetatable(JOBID, {{
            __index = function(t, k)
                return {}
            end
        }})
        "#,
        UNKNOWN_JOB_SENTINEL
    ))
    .exec()
}

pub fn exec_chunk(lua: &Lua, src: &[u8]) -> mlua::Result<()> {
    lua.load(src).exec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_job_metatables_resolves_unknown_to_sentinel() -> mlua::Result<()> {
        let lua = new_vm()?;
        exec_chunk(&lua, b"JTtbl = { NOVICE = 0 }")?;
        install_job_metatables(&lua)?;

        let novice: i32 = lua.load("return JOBID.NOVICE").eval()?;
        assert_eq!(novice, 0);

        let missing: i32 = lua.load("return JOBID.MISSING").eval()?;
        assert_eq!(missing, UNKNOWN_JOB_SENTINEL);

        Ok(())
    }

    #[test]
    fn exec_chunk_accepts_non_ascii_bytes() -> mlua::Result<()> {
        let lua = new_vm()?;
        // Build a Lua string assignment containing EUC-KR bytes (>= 0x80)
        let mut chunk = b"x = \"".to_vec();
        chunk.extend_from_slice(&[0xC3, 0xCA]);
        chunk.extend_from_slice(b"\"");
        exec_chunk(&lua, &chunk)
    }
}
