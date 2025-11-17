use super::decompiler::LuaDecompiler;
use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::reflect::TypePath;

#[derive(Asset, TypePath)]
pub struct LuaBytecode {
    pub data: Vec<u8>,
    pub source: String,
}

pub struct LuaBytecodeLoader {
    decompiler: std::sync::Mutex<LuaDecompiler>,
}

impl Default for LuaBytecodeLoader {
    fn default() -> Self {
        Self {
            decompiler: std::sync::Mutex::new(LuaDecompiler::new()),
        }
    }
}

impl AssetLoader for LuaBytecodeLoader {
    type Asset = LuaBytecode;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let source = {
            let mut decompiler = self.decompiler.lock().unwrap();
            let decompiled = decompiler
                .decompile_bytecode(&bytes)
                .expect("CRITICAL: Failed to decompile Lua bytecode");
            decompiled.source.clone()
        };

        Ok(LuaBytecode {
            data: bytes,
            source,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["lub"]
    }
}
