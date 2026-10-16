use lifthrasir_data::effects::{AuthoredBlend, AuthoredEffect, AuthoredEffectError};

fn sample() -> AuthoredEffect {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(
            r#"(
            fps: 30,
            layers: [(
                textures: ["ro://effects/a.png", "ro://effects/b.png"],
                blend: Add,
                keys: [
                    (at: 2, quad: (64.0, 128.0), color: (255, 128, 64, 0)),
                    (at: 12, xy: (-1.0, 1.0, 1.0, -1.0, -2.0, -2.0, 2.0, 2.0),
                     uv: (0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0),
                     offset: (5.0, -10.0), color: (64, 128, 255, 255),
                     angle: 15.0, texture_index: 1),
                ],
            )],
        )"#,
        )
        .unwrap()
}

#[test]
fn round_trips_layers_blends_raw_corners_and_defaults() {
    let mut effect = sample();
    assert_eq!(effect.layers[0].keys[0].offset, (0.0, 0.0));
    assert_eq!(effect.layers[0].keys[0].uv, None);
    assert_eq!(effect.layers[0].keys[0].angle, 0.0);
    assert_eq!(effect.layers[0].keys[0].texture_index, 0);
    for blend in [AuthoredBlend::Blend, AuthoredBlend::Multiply] {
        let mut layer = effect.layers[0].clone();
        layer.blend = blend;
        effect.layers.push(layer);
    }
    effect.validate().unwrap();
    let serialized = ron::to_string(&effect).unwrap();
    let restored: AuthoredEffect = ron::from_str(&serialized).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, effect);

    let options =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let restored: AuthoredEffect = options
        .from_str(&options.to_string(&effect).unwrap())
        .unwrap();
    assert_eq!(restored, effect);
}

#[test]
fn rejects_invalid_layers_and_keys_with_original_context() {
    use AuthoredEffectError::*;
    let valid = sample();
    let mut effect = valid.clone();
    effect.layers[0].textures.clear();
    assert_eq!(effect.validate(), Err(NoTextures { layer: 0 }));
    effect = valid.clone();
    effect.layers[0].keys.clear();
    assert_eq!(effect.validate(), Err(NoKeys { layer: 0 }));
    for at in [1, 2] {
        effect = valid.clone();
        effect.layers[0].keys[1].at = at;
        assert_eq!(effect.validate(), Err(UnsortedKeys { layer: 0, key: 1 }));
    }
    effect = valid.clone();
    effect.layers[0].keys[1].texture_index = 2;
    assert_eq!(
        effect.validate(),
        Err(TextureIndexOutOfRange {
            layer: 0,
            key: 1,
            index: 2,
            len: 2,
        })
    );
    for xy in [None, Some([0.0; 8])] {
        effect = valid.clone();
        effect.layers[0].keys[0].quad = xy.map(|_| (64.0, 128.0));
        effect.layers[0].keys[0].xy = xy;
        assert_eq!(effect.validate(), Err(QuadXyAmbiguous { layer: 0, key: 0 }));
    }

    effect = valid.clone();
    let mut invalid = valid.layers[0].clone();
    invalid.keys[1].texture_index = 5;
    effect.layers.push(invalid);
    assert_eq!(
        effect.validate(),
        Err(TextureIndexOutOfRange {
            layer: 1,
            key: 1,
            index: 5,
            len: 2,
        })
    );
}

#[test]
fn rejects_empty_effect_without_bevy() {
    let effect = AuthoredEffect {
        fps: 30,
        layers: vec![],
    };
    assert_eq!(effect.validate(), Err(AuthoredEffectError::NoLayers));
}
