use luadec::bytecode_reader::{lua_bytecode, LuaChunk, LuaConstant};

/// luadec's `write_constant` dumps a string constant as `"` + raw bytes + `"` with no
/// escaping. iteminfo descriptions contain `"`, `\`, and newlines, so the decompiled Lua
/// is malformed. We read the (length-prefixed, unambiguous) constant pool from luadec's
/// public bytecode reader and rewrite each mis-escaped literal in a single pass.
///
/// ponytail: this matches the broken literal as TEXT (`"` + raw + `"`). In theory a constant
/// whose value coincides with the source's inter-literal bytes (e.g. `a", "b`) could match a
/// phantom span across two adjacent literals. Not observed in real iteminfo data; the converter
/// logs the item count so a corrupted parse would show up. Re-lexing positionally would fix it
/// at much higher cost — deferred unless the count comes out wrong.
pub fn escape_string_constants(src: Vec<u8>, bytecode: &[u8]) -> anyhow::Result<Vec<u8>> {
    let (_, bc) = lua_bytecode(bytecode).map_err(|e| anyhow::anyhow!("parse bytecode: {e:?}"))?;

    let mut constants = Vec::new();
    collect_strings(&bc.main_chunk, &mut constants);

    let (broken, fixed): (Vec<Vec<u8>>, Vec<Vec<u8>>) = constants
        .iter()
        .filter(|bytes| bytes.iter().any(|&b| is_breaker(b)))
        .map(|bytes| (broken_literal(bytes), fixed_literal(bytes)))
        .unzip();

    if broken.is_empty() {
        return Ok(src);
    }

    let ac = aho_corasick::AhoCorasick::builder()
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build(&broken)?;

    Ok(ac.replace_all_bytes(&src, &fixed))
}

fn collect_strings(chunk: &LuaChunk, out: &mut Vec<Vec<u8>>) {
    for c in &chunk.constants {
        if let LuaConstant::String(bytes) = c {
            out.push(bytes.clone());
        }
    }
    for proto in &chunk.prototypes {
        collect_strings(proto, out);
    }
}

fn is_breaker(b: u8) -> bool {
    b == b'"' || b == b'\\' || b < 0x20
}

/// Exactly what luadec emitted: `"` + raw constant bytes + `"`.
fn broken_literal(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.push(b'"');
    out.extend_from_slice(bytes);
    out.push(b'"');
    out
}

/// A valid Lua literal: escape `"`, `\`, `\n`, `\r`, and other control bytes as `\NNN`.
/// Bytes >= 0x80 are left raw so EUC-KR survives to `decode_euckr`.
fn fixed_literal(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.push(b'"');
    for &b in bytes {
        match b {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b if b < 0x20 => out.extend_from_slice(format!("\\{b:03}").as_bytes()),
            b => out.push(b),
        }
    }
    out.push(b'"');
    out
}

#[cfg(test)]
mod tests {
    use super::escape_string_constants;

    fn dump(script: &str) -> Vec<u8> {
        let lua = mlua::Lua::new();
        lua.load(script).into_function().unwrap().dump(false)
    }

    #[test]
    fn round_trips_quote_backslash_newline() {
        let bc = dump("x = \"a\\\"b\\\\c\\nd\"");
        let raw = crate::decompile::decompile(&bc).unwrap();
        let fixed = escape_string_constants(raw, &bc).unwrap();

        let lua = mlua::Lua::new();
        lua.load(&fixed[..]).exec().unwrap();
        let x: String = lua.globals().get("x").unwrap();
        assert_eq!(x, "a\"b\\c\nd");
    }

    #[test]
    fn euc_kr_bytes_left_raw() {
        // Constant value = EUC-KR bytes 0xC3 0xCA (decimal 195 202) plus a quote.
        // mlua's loader resolves the \NNN escapes into raw constant-pool bytes.
        let bc = dump("x = \"\\195\\202\\\"\"");
        let raw = crate::decompile::decompile(&bc).unwrap();
        let fixed = escape_string_constants(raw, &bc).unwrap();

        // The >= 0x80 bytes must appear byte-identical (the `"` between them must be escaped).
        assert!(
            fixed.windows(2).any(|w| w == [0xC3u8, 0xCA]),
            "EUC-KR bytes not preserved verbatim"
        );

        let vm = mlua::Lua::new();
        vm.load(&fixed[..]).exec().unwrap();
        let x: mlua::String = vm.globals().get("x").unwrap();
        assert_eq!(x.as_bytes().as_ref(), &[0xC3, 0xCA, b'"']);
    }

    #[test]
    fn no_breaker_unchanged() {
        let bc = dump("x = \"plain ascii\"");
        let raw = crate::decompile::decompile(&bc).unwrap();
        let fixed = escape_string_constants(raw.clone(), &bc).unwrap();
        assert_eq!(fixed, raw);
    }
}
