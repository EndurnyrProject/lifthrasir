use bevy::prelude::*;
use lifthrasir_assets::catalogs::effect::EffectDataAsset;
use lifthrasir_assets::effects::authored::{AuthoredEffect, convert_authored_effect};
use lifthrasir_data::GroundAnchor;
use std::path::Path;

#[test]
fn performance_artwork_renders_one_centered_motif_per_cell() {
    let catalog: EffectDataAsset =
        ron::from_str(include_str!("../../../assets/data/ron/effects.ron")).unwrap();
    let options =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    for descriptor in catalog.0.skill_units.values() {
        if descriptor.ground_anchor != GroundAnchor::Footprint {
            continue;
        }
        let name = descriptor.str_name().expect("performance artwork");
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/data/effects")
            .join(name);
        let source = std::fs::read_to_string(path).unwrap();
        let effect: AuthoredEffect = options.from_str(&source).unwrap();
        let loaded = convert_authored_effect(&effect, |_| Handle::default()).unwrap();
        assert_eq!(
            loaded.layers.len(),
            1,
            "{name} must not repeat a cluster per cell"
        );
        for frame in &loaded.layers[0].frames {
            assert_eq!(frame.offset.x, 319.0, "{name} stays over its cell");
        }
    }
}
