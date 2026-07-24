# Animation (Bevy 0.19)

Distilled from `examples/animation/` in the Bevy 0.19 tree. All snippets use 0.19 APIs (note: `AnimatedBy` replaces the old `AnimationTarget { player, id }` struct; scenes load via `WorldAssetRoot` / `WorldInstanceReady` instead of `SceneRoot` / `SceneInstanceReady`).

## Core model

- **`AnimationClip`** (asset): a set of curves, each bound to an **`AnimationTargetId`** (stable hash of a `Name` path). Also carries timed **animation events**.
- **`AnimationGraph`** (asset): DAG of nodes — clip nodes, blend nodes, additive-blend nodes — each with a weight and mask. `AnimationGraphHandle(handle)` component links it to a player entity.
- **`AnimationPlayer`** (component): plays graph nodes by `AnimationNodeIndex`. Bevy evaluates everything automatically each frame; no user system needed.
- **Targets**: an animated entity carries `AnimationTargetId` + `AnimatedBy(player_entity)` components. glTF loading inserts these for you; code-built scenes insert them manually.

## AnimationPlayer + AnimationGraph basics

`examples/animation/animated_transform.rs`, `examples/animation/animated_mesh.rs`

Single clip → graph → player:

```rust
let (graph, node_index) = AnimationGraph::from_clip(clip_handle); // or from_clips([...]) -> Vec<AnimationNodeIndex>
let mut player = AnimationPlayer::default();
player.play(node_index).repeat();
commands.spawn((player, AnimationGraphHandle(graphs.add(graph)), /* mesh, Name, ... */));
```

Code-authored clips target entities by `Name` path; each animated entity needs the id + back-pointer:

```rust
let target_id = AnimationTargetId::from_name(&Name::new("planet"));
// nested: AnimationTargetId::from_names([planet, orbit].iter())
animation.add_curve_to_target(target_id, AnimatableCurve::new(
    animated_field!(Transform::translation),
    UnevenSampleAutoCurve::new([0.0, 1.0, 2.0].into_iter().zip([v0, v1, v0]))?,
));
// on the entity:
commands.entity(e).insert((target_id, AnimatedBy(player_entity)));
```

Multiple curves may target the same id (e.g. translation + rotation). For seamless looping, last keyframe = first.

For glTF: `asset_server.load(GltfAssetLabel::Animation(2).from_asset("models/animated/Fox.glb"))` loads a clip; spawn the scene with `WorldAssetRoot(scene_handle)` and `.observe(...)` on `WorldInstanceReady` — the loader auto-spawns an `AnimationPlayer` on the skeleton root; find it via `children.iter_descendants(...)` and insert `AnimationGraphHandle` there (`animated_mesh.rs`).

## Playback control (pause / speed / seek / repeat)

`examples/animation/animated_mesh_control.rs`

`player.animation_mut(node_index)` returns an `ActiveAnimation`:

```rust
let anim = player.animation_mut(index).unwrap();
anim.pause(); anim.resume(); anim.is_paused();
anim.set_speed(anim.speed() * 1.2);
anim.seek_to(anim.seek_time() + 0.1);
anim.set_repeat(RepeatAnimation::Count(2)).replay(); // or RepeatAnimation::Forever
```

`player.playing_animations()` iterates `(&AnimationNodeIndex, &ActiveAnimation)`; `player.is_playing_animation(idx)`.

## Transitions (cross-fade)

`examples/animation/animated_mesh_control.rs`

`AnimationTransitions` component manages fade in/out between clips. **Always start animations through it, never directly via the player** — it owns weight management:

```rust
let mut transitions = AnimationTransitions::new();
transitions.play(&mut player, animations[0], Duration::ZERO).repeat();
commands.entity(entity)
    .insert(AnimationGraphHandle(graph_handle.clone()))
    .insert(transitions);
// later, cross-fade over 250 ms:
transitions.play(&mut player, animations[next], Duration::from_millis(250)).repeat();
```

## Blending with graph structure

`examples/animation/animation_graph.rs`

Blend nodes weight their children; effective weight multiplies down the tree:

```rust
let mut graph = AnimationGraph::new();
let blend = graph.add_blend(0.5, graph.root);            // blend node, weight 0.5, parent = root
graph.add_clip(idle_clip, 1.0, graph.root);              // clip directly under root
graph.add_clip(walk_clip, 1.0, blend);
graph.add_clip(run_clip, 1.0, blend);
```

Play all clip nodes simultaneously, then drive per-animation weights at runtime with `active_animation.set_weight(w)`. Graphs are serializable: `SerializedAnimationGraph::try_from(graph)` → RON, loadable as an asset (`assets/animation_graphs/Fox.animgraph.ron`).

## Masks (per-bone animation toggling)

`examples/animation/animation_masks.rs`

Mask groups (0–63) are sets of targets; a node's `mask` bitfield **silences** the groups whose bits are set on that node:

```rust
let mut graph = AnimationGraph::new();
let blend = graph.add_additive_blend(1.0, graph.root);   // additive blending also exists
let node = graph.add_clip_with_mask(clip, 0x3f, 1.0, blend); // masked out of groups 0-5
graph.add_target_to_mask_group(target_id, group_index);      // assign bones to groups
// runtime toggle:
graph.get_mut(node).unwrap().mask &= !(1 << group_id);   // unmute group for this clip
graph.get_mut(node).unwrap().mask |= 1 << group_id;      // mute
```

Targets not in any group play all clips at once — the example strips `AnimationTargetId`/`AnimatedBy` from those to avoid it.

## Animating arbitrary properties

`examples/animation/animated_ui.rs`, `examples/animation/animated_transform.rs`

Two routes inside `AnimatableCurve::new(property, curve)`:

1. **`animated_field!`** — any `Reflect` component field whose type is `Animatable`: `animated_field!(Transform::scale)`, `animated_field!(UiTransform::rotation)` (yes, UI animates; `Rot2` slerps).
2. **Custom `AnimatableProperty`** — arbitrary accessor logic, e.g. animating the `Srgba` inside `TextColor`:

```rust
#[derive(Clone)]
struct TextColorProperty;

impl AnimatableProperty for TextColorProperty {
    type Property = Srgba;
    fn evaluator_id(&self) -> EvaluatorId<'_> { EvaluatorId::Type(TypeId::of::<Self>()) }
    fn get_mut<'a>(&self, entity: &'a mut AnimationEntityMut)
        -> Result<&'a mut Srgba, AnimationEvaluationError> {
        let text_color = entity.get_mut::<TextColor>()
            .ok_or(AnimationEvaluationError::ComponentNotPresent(TypeId::of::<TextColor>()))?
            .into_inner();
        match text_color.0 {
            Color::Srgba(ref mut c) => Ok(c),
            _ => Err(AnimationEvaluationError::PropertyNotPresent(TypeId::of::<Srgba>())),
        }
    }
}
// use: AnimatableCurve::new(TextColorProperty, AnimatableKeyframeCurve::new(times.zip(colors))?)
```

Curve backends: `AnimatableKeyframeCurve` (even use), `UnevenSampleAutoCurve` (uneven keyframes), or **any `Curve<T>`** — including `EasingCurve` (below).

## Animation events

`examples/animation/animation_events.rs`, `examples/animation/animated_mesh_events.rs`

Events are `#[derive(AnimationEvent, Clone)]` types embedded in a clip at a time; they fire as observer triggers when playback crosses that time:

```rust
#[derive(AnimationEvent, Clone)]
struct SetMessage { value: String, color: Color }

animation.set_duration(2.0);            // only needed if longer than last event
animation.add_event(1.0, SetMessage { value: "BYE".into(), color: CRIMSON.into() });
app.add_observer(|msg: On<SetMessage>, ...| { /* react */ });
```

Target-scoped events fire *on the target entity* (great for footsteps — `animated_mesh_events.rs` spawns dust at each fox foot):

```rust
#[derive(AnimationEvent, Reflect, Clone)]
struct Step;
clip.add_event_to_target(foot_target_id, 0.625, Step); // time = frame / fps
// observer reads the entity: transforms.get(step.trigger().target)?
```

You can mutate already-loaded glTF clips: resolve the clip handle from the graph node (`AnimationNodeType::Clip(handle)` → `clips.get_mut`).

## Easing: EaseFunction + EasingCurve + curve API

`examples/animation/eased_motion.rs`, `examples/animation/easing_functions.rs`

`EasingCurve::new(start, end, ease_fn)` builds a `Curve<T>` over [0, 1] for any `Ease` type (f32, Vec3, Quat, Rot2...). ~42 `EaseFunction` variants: `{Sine,Quadratic,Cubic,Quartic,Quintic,SmoothStep,SmootherStep,Circular,Exponential,Elastic,Back,Bounce}{In,Out,InOut}`, plus `Linear`, `Steps(n, JumpAt::{Start,End,Both,None})`, `Elastic(omega)`.

Curve adaptors compose, and the result feeds straight into an `AnimatableCurve`:

```rust
let translation_curve = EasingCurve::new(vec3(-6., 2., 0.), vec3(6., 2., 0.), EaseFunction::CubicInOut)
    .reparametrize_linear(interval(0.0, 3.0).unwrap()).unwrap() // stretch [0,1] to 3 s
    .ping_pong().unwrap();                                      // there and back: 6 s total
clip.add_curve_to_target(target_id,
    AnimatableCurve::new(animated_field!(Transform::translation), translation_curve));
```

Ad-hoc sampling (no clip/player needed): `EasingCurve::new(0.0, 1.0, f).sample(t)`; other API seen: `.by_ref()`, `.graph()`, `.map(...)`, `.domain().spaced_points(n)` (used to plot curves with gizmos in `easing_functions.rs`).

## Color animation (curves + mixing)

`examples/animation/color_animation.rs`

Only (perceptually/physically) linear spaces implement `VectorSpace` and support spline curves: `LinearRgba`, `Oklaba`, `Laba`, `Srgba`\*, `Xyza` — `CubicBezier::new([points]).to_curve()?.position(t)`. Non-linear spaces (`Hsla`, `Oklcha`) animate via the `Mix` trait: `a.mix(&b, t)` (works in any space). Manual per-frame system, not the AnimationPlayer.

## Morph targets (brief)

`examples/animation/morph_targets.rs`

Nothing special: glTF morph-target animations are ordinary clips — `AnimationGraph::from_clip(GltfAssetLabel::Animation(n)...)`, find the auto-spawned player on `WorldInstanceReady`, `player.play(index).repeat()`. Introspection: `mesh.morph_target_names()` on the loaded `Mesh` asset. (`custom_skinned_mesh.rs` is the manual-skinning counterpart: `SkinnedMesh` + `SkinnedMeshInverseBindposes` + `ATTRIBUTE_JOINT_INDEX/WEIGHT`, joints driven by mutating joint `Transform`s directly.)

## Sprite-sheet frame animation

`examples/2d/sprite_animation.rs` (none in `examples/animation/`; see also `examples/2d/sprite_sheet.rs`)

Bevy has **no built-in sprite-frame animator** — the official example is a hand-rolled `Timer` system stepping `TextureAtlas.index`, i.e. structurally the same thing Lifthrasir's ACT-driven system already does:

```rust
fn execute_animations(time: Res<Time>, mut query: Query<(&mut AnimationConfig, &mut Sprite)>) {
    for (mut config, mut sprite) in &mut query {
        config.frame_timer.tick(time.delta());
        if config.frame_timer.just_finished()
            && let Some(atlas) = &mut sprite.texture_atlas
        {
            atlas.index = if atlas.index == config.last { config.first } else { atlas.index + 1 };
            config.frame_timer = AnimationConfig::timer_from_fps(config.fps);
        }
    }
}
```

Setup: `TextureAtlasLayout::from_grid(UVec2::splat(24), 7, 1, None, None)` + `Sprite { image, texture_atlas: Some(TextureAtlas { layout, index }), .. }`. If you wanted the graph system to drive frames instead, a custom `AnimatableProperty` over the atlas index with `EaseFunction::Steps` would be the idiomatic bridge.

## Gotchas

- `AnimationTransitions` and direct `player.play()` don't mix — pick one per player.
- A shorter curve in a clip does not loop until the longest curve finishes; split differing periods into separate clips.
- `AnimatedBy(player)` must point at the entity holding the `AnimationPlayer`, and every animated entity needs its `AnimationTargetId` component matching the clip's target hash.
- Event time from frame number: `time = frame / fps`.
