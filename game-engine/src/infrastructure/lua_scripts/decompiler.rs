use bevy::prelude::*;
use gag::Gag;
use luadec::{DecompileError, LuaDecompiler as LuaDecImpl};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DecompiledLua {
    pub source: String,
    pub original_size: usize,
}

pub struct LuaDecompiler {
    cache: HashMap<u64, Arc<DecompiledLua>>,
    decompiler: LuaDecImpl,
}

impl Default for LuaDecompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaDecompiler {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            decompiler: LuaDecImpl::new(),
        }
    }

    pub fn decompile_bytecode(
        &mut self,
        bytecode: &[u8],
    ) -> Result<Arc<DecompiledLua>, DecompileError> {
        let hash = Self::hash_bytecode(bytecode);

        if let Some(cached) = self.cache.get(&hash) {
            debug!("Using cached decompiled Lua (hash: {})", hash);
            return Ok(Arc::clone(cached));
        }

        debug!("Decompiling Lua bytecode ({} bytes)", bytecode.len());

        let raw_source = {
            let _stdout_redirect = Gag::stdout().ok();
            self.decompiler.decompile(bytecode)?
        };

        let source = Self::fix_decompiler_syntax(&raw_source);

        let decompiled = Arc::new(DecompiledLua {
            source,
            original_size: bytecode.len(),
        });

        self.cache.insert(hash, Arc::clone(&decompiled));

        debug!(
            "Successfully decompiled {} bytes to {} bytes of source",
            bytecode.len(),
            decompiled.source.len()
        );

        Ok(decompiled)
    }

    fn hash_bytecode(bytecode: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        bytecode.hash(&mut hasher);
        hasher.finish()
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    fn fix_decompiler_syntax(source: &str) -> String {
        static DOTTED_FIELD_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(\s+)\.([A-Z_][A-Z0-9_]*)\s*=").unwrap());
        static STRING_INDEX_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"(JOBID|jobtbl)\["([A-Z_][A-Z0-9_]*)"\]"#).unwrap());

        let fixed_dots = DOTTED_FIELD_REGEX.replace_all(source, "$1$2 =");

        STRING_INDEX_REGEX
            .replace_all(&fixed_dots, "$1.$2")
            .to_string()
    }
}
