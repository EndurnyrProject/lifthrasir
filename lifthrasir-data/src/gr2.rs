//! Standard glTF contract for converted RO GR2 actors.

pub const IDLE: &str = "idle";
pub const WALK: &str = "walk";
pub const ATTACK: &str = "attack";
pub const HIT: &str = "hit";
pub const DEAD: &str = "dead";
pub const EMBLEM_MATERIAL: &str = "guild_emblem";
pub const GUILD_FLAG_JOB_ID: u32 = 722;

/// Map a GR2 basename to its normalized runtime GLB path.
pub fn model_asset_path(source_name: &str) -> Option<String> {
    let name = source_name.to_lowercase();
    let stem = name.strip_suffix(".gr2")?;
    if stem.is_empty() || stem.contains(['/', '\\', ':', '#', '?']) || matches!(stem, "." | "..") {
        return None;
    }
    Some(format!("ro://models/3dmob/{stem}.glb"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_models_without_creating_sprite_paths() {
        assert_eq!(
            model_asset_path("Guildflag90_1.GR2").as_deref(),
            Some("ro://models/3dmob/guildflag90_1.glb")
        );
        assert_eq!(
            model_asset_path("Aguardian90_8.gr2").as_deref(),
            Some("ro://models/3dmob/aguardian90_8.glb")
        );
        for name in ["poring", ".gr2", "../flag.gr2", "a\\flag.gr2", "flag#x.gr2"] {
            assert_eq!(model_asset_path(name), None);
        }
    }
}
