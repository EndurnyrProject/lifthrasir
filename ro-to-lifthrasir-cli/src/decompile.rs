use anyhow::Context;
use gag::Gag;
use luadec::{DecompileOptions, LuaDecompiler};
use regex::bytes::Regex;

pub fn decompile(bytecode: &[u8]) -> anyhow::Result<Vec<u8>> {
    let decompiler = LuaDecompiler::new();
    let options = DecompileOptions::default();
    let mut raw: Vec<u8> = Vec::new();

    let _gag = Gag::stdout().ok();
    decompiler
        .decompile_to_writer(bytecode, &mut raw, &options)
        .context("luadec decompilation failed")?;

    Ok(fix_decompiler_syntax(&raw))
}

fn fix_decompiler_syntax(bytes: &[u8]) -> Vec<u8> {
    let dotted = Regex::new(r"(\s+)\.([A-Za-z_][A-Za-z0-9_]*)\s*=").unwrap();
    let string_index =
        Regex::new(r#"(JOBID|jobtbl)\["([A-Z_][A-Z0-9_]*)"\]"#).unwrap();

    let after_dots = dotted.replace_all(bytes, &b"$1$2 ="[..]);
    string_index
        .replace_all(&after_dots, &b"$1.$2"[..])
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::fix_decompiler_syntax;

    #[test]
    fn dotted_field_uppercase() {
        let input = b"  .FOO = 1";
        let out = fix_decompiler_syntax(input);
        assert_eq!(out, b"  FOO = 1");
    }

    #[test]
    fn dotted_field_camel_case() {
        let input = b"  .fooBar = 1";
        let out = fix_decompiler_syntax(input);
        assert_eq!(out, b"  fooBar = 1");
    }

    #[test]
    fn string_index_replaced() {
        let input = br#"JOBID["FOO"]"#;
        let out = fix_decompiler_syntax(input);
        assert_eq!(out, b"JOBID.FOO");
    }

    #[test]
    fn euc_kr_bytes_survive() {
        // 0xC3 0xCA 0xBA 0xB8 is a valid EUC-KR sequence
        let mut input = b"before".to_vec();
        input.extend_from_slice(&[0xC3, 0xCA, 0xBA, 0xB8]);
        input.extend_from_slice(b"after");

        let out = fix_decompiler_syntax(&input);
        assert_eq!(out, input);
    }
}
