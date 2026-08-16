//! Server-driven illustration overlays (rAthena `cutin`).
//!
//! Aesir delivers the ordered [`CutinDisplayChanged`] stream; this module owns the
//! client half of the contract: filename trust validation, the single hidden
//! [`CutinRoot`] that acts as the pending-load cancellation token, presentation for
//! the five placement modes, viewport fit-down and anchoring, and the mode-4 local
//! close. [`drive_cutins`] is the sole owner of structural lifecycle commands, so
//! ingress, asset completion, and dismissal can never enqueue competing despawns.

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy::ui_widgets::Activate;
use bevy::window::PrimaryWindow;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use game_engine::core::state::GameState;
use net_contract::events::{CutinDisplayChanged, CutinPlacement};

use crate::theme;
use crate::theme::feathers_theme::{TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BORDER};
use crate::widgets::chrome::ignore_picking;
use crate::widgets::draggable::px_or_zero;

/// Canonical rAthena illustration directory inside the `ro://` namespace.
const CUTIN_DIR: &str = "ro://data/texture/유저인터페이스/illust/";

/// Cutins render above the world but below default-z HUD windows and dialogs.
const CUTIN_Z: i32 = -1;

/// Base mode-3 titlebar height; it scales with the presentation during fit-down.
const TITLEBAR_HEIGHT: f32 = 30.0;

/// Base mode-4 close-control side length; scales with the presentation fit factor.
const CLOSE_BUTTON_SIZE: f32 = 22.0;

/// Base mode-4 close-control inset from the top-right corner.
const CLOSE_BUTTON_OFFSET: f32 = 4.0;

/// Base mode-4 close glyph side length; scales with the presentation fit factor.
const CLOSE_GLYPH_SIZE: f32 = 12.0;

pub struct CutinPlugin;

impl Plugin for CutinPlugin {
    fn build(&self, app: &mut App) {
        // Runs in PostUpdate so every Update producer (the aesir adapter among them)
        // has already written this frame's Cutin messages before the lifecycle
        // consumes them; a buffered message can never leak into a later session.
        // Ordered before the UI prepare set so Node/Visibility mutations are visible
        // to the UI layout pipeline in the same frame. Chained so a finalized
        // CutinLayout is visible to layout_cutin the frame drive_cutins queues it.
        app.add_systems(
            PostUpdate,
            (drive_cutins, layout_cutin)
                .chain()
                .before(UiSystems::Prepare),
        );
    }
}

/// The single cutin root; its entity id is the cancellation token for pending loads.
#[derive(Component, Debug)]
struct CutinRoot {
    placement: CutinPlacement,
    dismissed: bool,
}

/// Present while the bitmap is still loading; swapped for [`CutinLayout`] on success.
#[derive(Component, Debug)]
struct PendingCutin {
    handle: Handle<Image>,
    path: String,
}

/// Source dimensions plus whether a centered cutin has already been positioned.
#[derive(Component, Debug)]
struct CutinLayout {
    source_size: Vec2,
    positioned: bool,
}

impl CutinLayout {
    fn new(source_size: Vec2) -> Self {
        Self {
            source_size,
            positioned: false,
        }
    }
}

#[derive(Component, Default, Clone, Debug)]
struct CutinImage;

#[derive(Component, Default, Clone, Debug)]
struct CutinTitlebar;

#[derive(Component, Default, Clone, Debug)]
struct CutinDragHandle;

#[derive(Component, Default, Clone, Debug)]
struct CutinClose;

#[derive(Component, Default, Clone, Debug)]
struct CutinCloseGlyph;

/// Why a server-provided cutin image name is not a canonical illustration filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CutinPathError {
    Empty,
    PathSeparator,
    Traversal,
    LabelSeparator,
    BadExtension,
}

/// Resolves a server image name to a canonical illustration BMP path, or rejects it.
fn cutin_asset_path(image: &str) -> Result<String, CutinPathError> {
    if image.trim().is_empty() {
        return Err(CutinPathError::Empty);
    }
    if image.contains('/') || image.contains('\\') {
        return Err(CutinPathError::PathSeparator);
    }
    if image.contains('#') {
        return Err(CutinPathError::LabelSeparator);
    }
    if image.contains("..") {
        return Err(CutinPathError::Traversal);
    }

    let bytes = image.as_bytes();
    let has_bmp = bytes.len() >= 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".bmp");
    let stem = if has_bmp {
        &image[..image.len() - 4]
    } else {
        image
    };

    if stem.trim().is_empty() {
        return Err(CutinPathError::Empty);
    }
    // A dotted name without a final `.bmp` carries some other extension; a dotted
    // stem under `.bmp` (e.g. `portrait.v2.bmp`) is a legitimate basename.
    if !has_bmp && image.contains('.') {
        return Err(CutinPathError::BadExtension);
    }

    Ok(format!("{CUTIN_DIR}{stem}.bmp"))
}

/// A resolved, valid same-frame action from the incoming message stream.
enum ResolvedAction {
    Show {
        path: String,
        placement: CutinPlacement,
    },
    Clear,
}

fn resolve_action_or_warn(event: &CutinDisplayChanged) -> Option<ResolvedAction> {
    match event {
        CutinDisplayChanged::Show { image, placement } => match cutin_asset_path(image) {
            Ok(path) => Some(ResolvedAction::Show {
                path,
                placement: *placement,
            }),
            Err(error) => {
                warn!("ignoring invalid cutin image name {image:?}: {error:?}");
                None
            }
        },
        CutinDisplayChanged::Clear => Some(ResolvedAction::Clear),
    }
}

/// Sole owner of cutin structural commands.
fn drive_cutins(
    mut events: MessageReader<CutinDisplayChanged>,
    state: Res<State<GameState>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
    roots: Query<(Entity, &CutinRoot)>,
    pending: Query<(Entity, &PendingCutin), With<CutinRoot>>,
) {
    if state.get() != &GameState::InGame {
        events.clear();
        return;
    }

    let action = events.read().filter_map(resolve_action_or_warn).last();
    if let Some(action) = action {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        match action {
            ResolvedAction::Clear => {}
            ResolvedAction::Show { path, placement } => {
                let handle = asset_server.load(path.clone());
                spawn_hidden_root(&mut commands, path, handle, placement);
            }
        }
        return;
    }

    if let Ok((entity, root)) = roots.single()
        && root.dismissed
    {
        commands.entity(entity).despawn();
        return;
    }

    let Ok((entity, pending)) = pending.single() else {
        return;
    };
    if let Some(image) = images.get(&pending.handle) {
        let size = image.size_f32();
        if size.x > 0.0 && size.y > 0.0 && size.x.is_finite() && size.y.is_finite() {
            commands
                .entity(entity)
                .remove::<PendingCutin>()
                .insert(CutinLayout::new(size));
        } else {
            warn!(
                "discarding cutin '{}' with invalid dimensions {size:?}",
                pending.path
            );
            commands.entity(entity).despawn();
        }
    } else if let LoadState::Failed(error) = asset_server.load_state(&pending.handle) {
        warn!("failed to load cutin '{}': {error:?}", pending.path);
        commands.entity(entity).despawn();
    }
}

fn spawn_hidden_root(
    commands: &mut Commands,
    path: String,
    handle: Handle<Image>,
    placement: CutinPlacement,
) {
    let mut entity = match placement {
        CutinPlacement::BottomLeft | CutinPlacement::BottomCenter | CutinPlacement::BottomRight => {
            commands.spawn_scene(static_cutin(path.clone()))
        }
        CutinPlacement::CenterWindow => commands.spawn_scene(windowed_cutin(path.clone())),
        CutinPlacement::CenterChromeless => commands.spawn_scene(chromeless_cutin(path.clone())),
    };
    entity.insert((
        CutinRoot {
            placement,
            dismissed: false,
        },
        PendingCutin { handle, path },
        DespawnOnExit(GameState::InGame),
    ));
}

fn static_cutin(path: String) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: px(0.0),
            height: px(0.0),
        }
        Visibility::Hidden
        GlobalZIndex(CUTIN_Z)
        ignore_picking()
        Children [(
            CutinImage
            ImageNode { image: {path} }
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: px(0.0),
                height: px(0.0),
            }
            ignore_picking()
        )]
    }
}

fn windowed_cutin(path: String) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: px(0.0),
            height: px(0.0),
        }
        Visibility::Hidden
        GlobalZIndex(CUTIN_Z)
        ignore_picking()
        on(drag_cutin)
        Children [
            (
                CutinTitlebar
                CutinDragHandle
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    width: px(0.0),
                    height: px(TITLEBAR_HEIGHT),
                    border: {UiRect { bottom: Val::Px(1.0), ..default() }},
                }
                ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
                ThemeBorderColor({TOKEN_WINDOW_BORDER})
                Pickable
            ),
            (
                CutinImage
                ImageNode { image: {path} }
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    width: px(0.0),
                    height: px(0.0),
                }
                ignore_picking()
            ),
        ]
    }
}

fn chromeless_cutin(path: String) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: px(0.0),
            height: px(0.0),
            overflow: {Overflow::clip()},
        }
        Visibility::Hidden
        GlobalZIndex(CUTIN_Z)
        ignore_picking()
        on(drag_cutin)
        Children [
            (
                CutinImage
                CutinDragHandle
                ImageNode { image: {path} }
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    width: px(0.0),
                    height: px(0.0),
                }
                Pickable
            ),
            (
                CutinClose
                @FeathersButton {
                    @caption: bsn! {
                        CutinCloseGlyph
                        ImageNode {
                            image: {format!("{}{}.svg", theme::ICON_DIR, "close")},
                            color: theme::TEXT_DIM,
                        }
                        Node { width: px(CLOSE_GLYPH_SIZE), height: px(CLOSE_GLYPH_SIZE) }
                        ignore_picking()
                    }
                }
                Node {
                    position_type: PositionType::Absolute,
                    top: px(CLOSE_BUTTON_OFFSET),
                    right: px(CLOSE_BUTTON_OFFSET),
                    width: px(CLOSE_BUTTON_SIZE),
                    height: px(CLOSE_BUTTON_SIZE),
                    // Feathers' 8px button padding inflates the laid-out box beyond the
                    // factor-scaled width at extreme fit-down; zero it so the button's
                    // computed size matches the fitted dimensions.
                    padding: UiRect::ZERO,
                    min_width: px(0.0),
                    min_height: px(0.0),
                }
                ZIndex(1)
                on(dismiss_cutin)
            ),
        ]
    }
}

/// Fitted presentation sizes. The mode-3 titlebar participates in the root height.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FittedCutin {
    image: Vec2,
    root: Vec2,
    titlebar: f32,
    factor: f32,
}

fn fitted_size(source: Vec2, viewport: Vec2, titlebar: f32) -> FittedCutin {
    let base = Vec2::new(source.x, source.y + titlebar);
    let factor = (viewport.x / base.x)
        .min(viewport.y / base.y)
        .clamp(0.0, 1.0);
    FittedCutin {
        image: source * factor,
        root: base * factor,
        titlebar: titlebar * factor,
        factor,
    }
}

/// Collapses zero, negative, and non-finite UI scales to the identity scale `1.0`.
fn sanitize_ui_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn canonical_origin(placement: CutinPlacement, viewport: Vec2, size: Vec2) -> Vec2 {
    match placement {
        CutinPlacement::BottomLeft => Vec2::new(0.0, viewport.y - size.y),
        CutinPlacement::BottomCenter => Vec2::new((viewport.x - size.x) * 0.5, viewport.y - size.y),
        CutinPlacement::BottomRight => Vec2::new(viewport.x - size.x, viewport.y - size.y),
        CutinPlacement::CenterWindow | CutinPlacement::CenterChromeless => {
            Vec2::new((viewport.x - size.x) * 0.5, (viewport.y - size.y) * 0.5)
        }
    }
}

fn clamp_origin(origin: Vec2, viewport: Vec2, size: Vec2) -> Vec2 {
    origin.clamp(Vec2::ZERO, (viewport - size).max(Vec2::ZERO))
}

type CutinRootQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CutinRoot,
        &'static mut CutinLayout,
        &'static mut Node,
        &'static mut Visibility,
    ),
    (Without<CutinImage>, Without<CutinTitlebar>),
>;

type CutinImageNodeQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (With<CutinImage>, Without<CutinRoot>, Without<CutinTitlebar>),
>;

type CutinTitlebarNodeQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (With<CutinTitlebar>, Without<CutinRoot>, Without<CutinImage>),
>;

type CutinCloseNodeQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<CutinClose>,
        Without<CutinRoot>,
        Without<CutinImage>,
        Without<CutinTitlebar>,
    ),
>;

type CutinCloseGlyphNodeQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<CutinCloseGlyph>,
        Without<CutinRoot>,
        Without<CutinImage>,
        Without<CutinTitlebar>,
        Without<CutinClose>,
    ),
>;

/// Mutation-only layout pass: fit, anchor/clamp, size, and reveal the cutin.
fn layout_cutin(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut roots: CutinRootQuery,
    mut images: CutinImageNodeQuery,
    mut titlebars: CutinTitlebarNodeQuery,
    mut closes: CutinCloseNodeQuery,
    mut glyphs: CutinCloseGlyphNodeQuery,
) {
    let viewport = window.size() / sanitize_ui_scale(ui_scale.0);

    for (root, mut layout, mut node, mut visibility) in &mut roots {
        if root.dismissed {
            continue;
        }

        let titlebar_height = if root.placement == CutinPlacement::CenterWindow {
            TITLEBAR_HEIGHT
        } else {
            0.0
        };
        let fitted = fitted_size(layout.source_size, viewport, titlebar_height);

        let centered = matches!(
            root.placement,
            CutinPlacement::CenterWindow | CutinPlacement::CenterChromeless
        );
        let origin = if layout.positioned && centered {
            clamp_origin(
                Vec2::new(px_or_zero(node.left), px_or_zero(node.top)),
                viewport,
                fitted.root,
            )
        } else {
            canonical_origin(root.placement, viewport, fitted.root)
        };

        node.left = Val::Px(origin.x);
        node.top = Val::Px(origin.y);
        node.width = Val::Px(fitted.root.x);
        node.height = Val::Px(fitted.root.y);
        layout.positioned = true;
        *visibility = Visibility::Inherited;

        for mut image in &mut images {
            image.top = Val::Px(fitted.titlebar);
            image.width = Val::Px(fitted.image.x);
            image.height = Val::Px(fitted.image.y);
        }
        for mut titlebar in &mut titlebars {
            titlebar.width = Val::Px(fitted.root.x);
            titlebar.height = Val::Px(fitted.titlebar);
        }
        for mut close in &mut closes {
            let size = CLOSE_BUTTON_SIZE * fitted.factor;
            let offset = CLOSE_BUTTON_OFFSET * fitted.factor;
            close.top = Val::Px(offset);
            close.right = Val::Px(offset);
            close.width = Val::Px(size);
            close.height = Val::Px(size);
        }
        for mut glyph in &mut glyphs {
            let size = CLOSE_GLYPH_SIZE * fitted.factor;
            glyph.width = Val::Px(size);
            glyph.height = Val::Px(size);
        }
    }
}

/// Moves the single root by the pointer delta in UI coordinates. The event bubbles
/// from either drag handle to the root, so a drag that starts on the mode-4 close
/// button (which has no handle marker) is rejected here.
fn drag_cutin(
    drag: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    handles: Query<(), With<CutinDragHandle>>,
    mut roots: Query<&mut Node, With<CutinRoot>>,
) {
    if handles.get(drag.original_event_target()).is_err() {
        return;
    }
    let scale = sanitize_ui_scale(ui_scale.0);
    let Ok(mut node) = roots.single_mut() else {
        return;
    };
    node.left = Val::Px(px_or_zero(node.left) + drag.delta.x / scale);
    node.top = Val::Px(px_or_zero(node.top) + drag.delta.y / scale);
}

/// Mode-4 local close: hides and flags the root without any network output or
/// structural command. `drive_cutins` owns the subsequent despawn.
fn dismiss_cutin(_: On<Activate>, mut roots: Query<(&mut CutinRoot, &mut Visibility)>) {
    if let Ok((mut root, mut visibility)) = roots.single_mut() {
        *visibility = Visibility::Hidden;
        root.dismissed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::io::memory::MemoryAssetReader;
    use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
    use bevy::asset::{AssetPlugin, RenderAssetUsages};
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    use bevy::state::app::StatesPlugin;
    use bevy::ui::IsDefaultUiCamera;
    use bevy::window::WindowResolution;
    use game_engine::infrastructure::assets::bmp_loader::BmpLoader;

    // ---- Pure helpers -----------------------------------------------------

    #[test]
    fn cutin_asset_path_accepts_extensionless_and_case_insensitive_bmp() {
        assert_eq!(
            cutin_asset_path("event_illust").unwrap(),
            "ro://data/texture/유저인터페이스/illust/event_illust.bmp"
        );
        assert_eq!(
            cutin_asset_path("한글일러스트").unwrap(),
            "ro://data/texture/유저인터페이스/illust/한글일러스트.bmp"
        );
        assert_eq!(
            cutin_asset_path("portrait.bmp").unwrap(),
            "ro://data/texture/유저인터페이스/illust/portrait.bmp"
        );
        assert_eq!(
            cutin_asset_path("PORTRAIT.BMP").unwrap(),
            "ro://data/texture/유저인터페이스/illust/PORTRAIT.bmp"
        );
        assert_eq!(
            cutin_asset_path("Mixed.BmP").unwrap(),
            "ro://data/texture/유저인터페이스/illust/Mixed.bmp"
        );
        assert_eq!(
            cutin_asset_path("portrait.v2.bmp").unwrap(),
            "ro://data/texture/유저인터페이스/illust/portrait.v2.bmp"
        );
        assert_eq!(
            cutin_asset_path("portrait.png.bmp").unwrap(),
            "ro://data/texture/유저인터페이스/illust/portrait.png.bmp"
        );
    }

    #[test]
    fn cutin_asset_path_rejects_invalid_names() {
        assert_eq!(cutin_asset_path(""), Err(CutinPathError::Empty));
        assert_eq!(cutin_asset_path("   "), Err(CutinPathError::Empty));
        assert_eq!(cutin_asset_path(".bmp"), Err(CutinPathError::Empty));
        assert_eq!(cutin_asset_path("   .bmp"), Err(CutinPathError::Empty));
        assert_eq!(
            cutin_asset_path("dir/portrait"),
            Err(CutinPathError::PathSeparator)
        );
        assert_eq!(
            cutin_asset_path("dir\\portrait"),
            Err(CutinPathError::PathSeparator)
        );
        assert_eq!(cutin_asset_path(".."), Err(CutinPathError::Traversal));
        assert_eq!(cutin_asset_path("..bmp"), Err(CutinPathError::Traversal));
        assert_eq!(
            cutin_asset_path("portrait#label"),
            Err(CutinPathError::LabelSeparator)
        );
        assert_eq!(
            cutin_asset_path("portrait.png"),
            Err(CutinPathError::BadExtension)
        );
        assert_eq!(
            cutin_asset_path("portrait.v2"),
            Err(CutinPathError::BadExtension)
        );
    }

    #[test]
    fn fitted_size_scales_down_proportionally_and_only_down() {
        let source = Vec2::new(400.0, 300.0);
        // Fits within a smaller viewport while preserving aspect ratio.
        let small = fitted_size(source, Vec2::new(200.0, 300.0), 0.0);
        assert!((small.image.x - 200.0).abs() < 1e-4);
        assert!((small.image.y - 150.0).abs() < 1e-4);
        // Never upscales beyond 1x.
        let large = fitted_size(source, Vec2::new(4000.0, 3000.0), 0.0);
        assert_eq!(large.image, source);
        assert_eq!(large.root, source);
    }

    #[test]
    fn fitted_size_includes_mode_three_titlebar() {
        let source = Vec2::new(400.0, 600.0);
        let fitted = fitted_size(source, Vec2::new(400.0, 640.0), TITLEBAR_HEIGHT);
        assert!((fitted.titlebar - TITLEBAR_HEIGHT).abs() < 1e-4);
        assert_eq!(fitted.image, source);
        assert_eq!(fitted.root, Vec2::new(400.0, 630.0));

        // A tiny viewport shrinks both image and titlebar uniformly.
        let tiny = fitted_size(source, Vec2::new(200.0, 160.0), TITLEBAR_HEIGHT);
        let factor = 160.0 / (600.0 + TITLEBAR_HEIGHT);
        assert!((tiny.image.y - 600.0 * factor).abs() < 1e-3);
        assert!((tiny.titlebar - TITLEBAR_HEIGHT * factor).abs() < 1e-3);
    }

    #[test]
    fn canonical_origin_places_bottom_anchors_and_centers() {
        let viewport = Vec2::new(800.0, 600.0);
        let size = Vec2::new(400.0, 300.0);
        assert_eq!(
            canonical_origin(CutinPlacement::BottomLeft, viewport, size),
            Vec2::new(0.0, 300.0)
        );
        assert_eq!(
            canonical_origin(CutinPlacement::BottomCenter, viewport, size),
            Vec2::new(200.0, 300.0)
        );
        assert_eq!(
            canonical_origin(CutinPlacement::BottomRight, viewport, size),
            Vec2::new(400.0, 300.0)
        );
        assert_eq!(
            canonical_origin(CutinPlacement::CenterWindow, viewport, size),
            Vec2::new(200.0, 150.0)
        );
        assert_eq!(
            canonical_origin(CutinPlacement::CenterChromeless, viewport, size),
            Vec2::new(200.0, 150.0)
        );
    }

    #[test]
    fn clamp_origin_keeps_origin_inside_the_viewport() {
        let viewport = Vec2::new(800.0, 600.0);
        let size = Vec2::new(400.0, 300.0);
        assert_eq!(
            clamp_origin(Vec2::new(-50.0, 400.0), viewport, size),
            Vec2::new(0.0, 300.0)
        );
        assert_eq!(
            clamp_origin(Vec2::new(700.0, -20.0), viewport, size),
            Vec2::new(400.0, 0.0)
        );
        // Oversized content clamps to zero rather than panicking.
        assert_eq!(
            clamp_origin(
                Vec2::new(10.0, 10.0),
                Vec2::new(100.0, 100.0),
                Vec2::new(200.0, 200.0)
            ),
            Vec2::ZERO
        );
    }

    #[test]
    fn fitted_size_composes_with_ui_scale_fit_down() {
        let source = Vec2::new(600.0, 400.0);
        let window = Vec2::new(800.0, 600.0);

        // At 80% and 100% the presentation still fits: no shrink.
        for scale in [0.8, 1.0] {
            let fitted = fitted_size(source, window / scale, 0.0);
            assert_eq!(
                fitted.image, source,
                "scale {scale} must not shrink a fitting image"
            );
        }

        // At 200% the UI viewport halves, so the image shrinks proportionally.
        let fitted = fitted_size(source, window / 2.0, 0.0);
        let factor = (400.0_f32 / 600.0_f32).min(300.0_f32 / 400.0_f32);
        assert!((fitted.image.x - 600.0 * factor).abs() < 1e-4);
        assert!((fitted.image.y - 400.0 * factor).abs() < 1e-4);
        assert!(fitted.image.x <= 400.0 && fitted.image.y <= 300.0);
    }

    #[test]
    fn sanitize_ui_scale_collapses_non_positive_and_non_finite() {
        assert_eq!(sanitize_ui_scale(f32::INFINITY), 1.0);
        assert_eq!(sanitize_ui_scale(f32::NAN), 1.0);
        assert_eq!(sanitize_ui_scale(0.0), 1.0);
        assert_eq!(sanitize_ui_scale(1.5), 1.5);
    }

    // ---- Lifecycle ---------------------------------------------------------

    fn lifecycle_app() -> App {
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Name("ro".into()),
            AssetSourceBuilder::new(|| Box::new(MemoryAssetReader::default())),
        );
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_state::<GameState>();
        app.init_asset::<Image>();
        app.register_asset_loader(BmpLoader);
        app.add_message::<CutinDisplayChanged>();
        app.insert_resource(UiScale(1.0));
        app.add_plugins(CutinPlugin);
        app
    }

    fn integration_app() -> App {
        let mut app = lifecycle_app();
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..default()
            },
            PrimaryWindow,
        ));
        app
    }

    fn ui_pipeline_app() -> App {
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Name("ro".into()),
            AssetSourceBuilder::new(|| Box::new(MemoryAssetReader::default())),
        );
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            bevy::input::InputPlugin,
            bevy::text::TextPlugin,
            bevy::ui::UiPlugin,
            bevy::window::WindowPlugin {
                primary_window: None,
                ..default()
            },
            bevy::picking::DefaultPickingPlugins,
        ));
        app.init_state::<GameState>();
        app.init_asset::<Image>();
        app.init_asset::<bevy::image::TextureAtlasLayout>();
        app.register_asset_loader(BmpLoader);
        app.add_message::<CutinDisplayChanged>();
        app.insert_resource(UiScale(1.0));
        app.add_plugins(CutinPlugin);
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..default()
            },
            PrimaryWindow,
        ));
        app.world_mut().spawn((
            Camera2d,
            IsDefaultUiCamera,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(800, 600),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                ..default()
            },
        ));
        app
    }

    fn write_show_once_in_update(
        mut wrote: Local<bool>,
        mut writer: MessageWriter<CutinDisplayChanged>,
    ) {
        if *wrote {
            return;
        }
        *wrote = true;
        writer.write(CutinDisplayChanged::Show {
            image: "event_illust".to_string(),
            placement: CutinPlacement::BottomLeft,
        });
    }

    fn enter_in_game(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
    }

    fn show(image: &str, placement: CutinPlacement) -> CutinDisplayChanged {
        CutinDisplayChanged::Show {
            image: image.to_string(),
            placement,
        }
    }

    fn write_cutin(app: &mut App, event: CutinDisplayChanged) {
        app.world_mut()
            .resource_mut::<Messages<CutinDisplayChanged>>()
            .write(event);
    }

    fn root_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<CutinRoot>>()
            .iter(app.world())
            .count()
    }

    fn pending_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&PendingCutin>()
            .iter(app.world())
            .count()
    }

    fn single_pending(app: &mut App) -> (Entity, Handle<Image>, String) {
        let (entity, pending) = app
            .world_mut()
            .query::<(Entity, &PendingCutin)>()
            .single(app.world())
            .unwrap();
        (entity, pending.handle.clone(), pending.path.clone())
    }

    fn fill_image(width: u32, height: u32) -> Image {
        Image::new_fill(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0u8; 4],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    fn zero_image() -> Image {
        Image::new_uninit(
            Extent3d {
                width: 0,
                height: 0,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    fn insert_image(app: &mut App, handle: &Handle<Image>, image: Image) {
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(handle, image)
            .unwrap();
    }

    fn set_ui_viewport(app: &mut App, width: u32, height: u32) {
        let mut windows = app.world_mut().query::<&mut Window>();
        windows
            .single_mut(app.world_mut())
            .unwrap()
            .resolution
            .set(width as f32, height as f32);
        let mut cameras = app.world_mut().query::<&mut Camera>();
        let mut camera = cameras.single_mut(app.world_mut()).unwrap();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(width, height),
            scale_factor: 1.0,
        });
    }

    fn node_bounds(app: &App, entity: Entity) -> Rect {
        let computed = app.world().get::<bevy::ui::ComputedNode>(entity).unwrap();
        let transform = app
            .world()
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .unwrap();
        Rect::from_center_size(transform.translation, computed.size())
    }

    fn assert_rect_contains(outer: Rect, inner: Rect) {
        assert!(outer.min.x <= inner.min.x + 1e-3);
        assert!(outer.min.y <= inner.min.y + 1e-3);
        assert!(outer.max.x >= inner.max.x - 1e-3);
        assert!(outer.max.y >= inner.max.y - 1e-3);
    }

    #[test]
    fn drive_ignores_events_outside_in_game() {
        let mut app = lifecycle_app();
        write_cutin(&mut app, show("event_illust", CutinPlacement::BottomLeft));
        app.update();
        assert_eq!(root_count(&mut app), 0);

        // The buffered event was consumed, not merely deferred.
        enter_in_game(&mut app);
        app.update();
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn invalid_filename_preserves_an_existing_visible_root() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        app.world_mut().spawn((
            CutinRoot {
                placement: CutinPlacement::BottomLeft,
                dismissed: false,
            },
            CutinLayout::new(Vec2::new(400.0, 300.0)),
            Node::default(),
            Visibility::Inherited,
        ));

        write_cutin(&mut app, show("../traversal", CutinPlacement::BottomLeft));
        app.update();

        assert_eq!(root_count(&mut app), 1);
        assert_eq!(pending_count(&mut app), 0);
    }

    #[test]
    fn last_valid_same_frame_action_wins() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        write_cutin(&mut app, show("first", CutinPlacement::BottomLeft));
        write_cutin(&mut app, show("../invalid", CutinPlacement::BottomRight));
        write_cutin(&mut app, show("last", CutinPlacement::BottomRight));
        app.update();

        assert_eq!(root_count(&mut app), 1);
        let (_, _, path) = single_pending(&mut app);
        assert!(path.ends_with("/last.bmp"));
    }

    #[test]
    fn show_replaces_the_previous_root_with_one_hidden_pending() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        write_cutin(&mut app, show("first", CutinPlacement::BottomLeft));
        app.update();
        let (first_root, _, _) = single_pending(&mut app);

        write_cutin(&mut app, show("second", CutinPlacement::CenterWindow));
        app.update();

        assert!(app.world().get_entity(first_root).is_err());
        assert_eq!(root_count(&mut app), 1);
        assert_eq!(pending_count(&mut app), 1);
        let (_, _, path) = single_pending(&mut app);
        assert!(path.ends_with("/second.bmp"));
    }

    #[test]
    fn repeated_image_still_replaces_the_root() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        write_cutin(&mut app, show("same", CutinPlacement::BottomLeft));
        app.update();
        let (first_root, _, _) = single_pending(&mut app);

        write_cutin(&mut app, show("same", CutinPlacement::BottomLeft));
        app.update();

        assert!(app.world().get_entity(first_root).is_err());
        assert_eq!(root_count(&mut app), 1);
        assert_eq!(pending_count(&mut app), 1);
    }

    #[test]
    fn clear_removes_the_root() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        write_cutin(&mut app, show("event_illust", CutinPlacement::BottomLeft));
        app.update();
        assert_eq!(root_count(&mut app), 1);

        write_cutin(&mut app, CutinDisplayChanged::Clear);
        app.update();
        assert_eq!(root_count(&mut app), 0);
        assert_eq!(pending_count(&mut app), 0);
    }

    #[test]
    fn drive_removes_a_dismissed_root() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        app.world_mut().spawn((
            CutinRoot {
                placement: CutinPlacement::CenterChromeless,
                dismissed: true,
            },
            CutinLayout::new(Vec2::new(400.0, 300.0)),
            Node::default(),
            Visibility::Hidden,
        ));
        app.update();
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn update_written_cutin_outside_in_game_is_consumed_before_next_session() {
        let mut app = lifecycle_app();
        app.add_systems(Update, write_show_once_in_update);
        app.update();
        assert_eq!(root_count(&mut app), 0);

        enter_in_game(&mut app);
        app.update();
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn update_written_cutin_in_game_renders_through_post_update() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        app.add_systems(Update, write_show_once_in_update);
        app.update();

        assert_eq!(root_count(&mut app), 1);
        assert_eq!(pending_count(&mut app), 1);
    }

    #[test]
    fn finalized_cutin_reflects_computed_layout_in_one_frame() {
        let mut app = ui_pipeline_app();
        enter_in_game(&mut app);

        write_cutin(&mut app, show("event_illust", CutinPlacement::BottomLeft));
        app.update();
        let (root, handle, _) = single_pending(&mut app);
        insert_image(&mut app, &handle, fill_image(400, 300));

        app.update();

        let computed = app.world().get::<bevy::ui::ComputedNode>(root).unwrap();
        assert!((computed.size().x - 400.0).abs() < 1e-3);
        assert!((computed.size().y - 300.0).abs() < 1e-3);
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn plugin_chain_finalizes_and_reveals_in_one_frame() {
        let mut app = integration_app();
        enter_in_game(&mut app);

        write_cutin(&mut app, show("event_illust", CutinPlacement::BottomLeft));
        app.update();
        let (root, handle, _) = single_pending(&mut app);

        // The scene's ImageNode and the PendingCutin share the same deduped handle.
        let image_handle = app
            .world_mut()
            .query_filtered::<&ImageNode, With<CutinImage>>()
            .iter(app.world())
            .next()
            .unwrap()
            .image
            .clone();
        assert_eq!(image_handle, handle);

        insert_image(&mut app, &handle, fill_image(400, 300));
        app.update();

        assert!(app.world().get::<PendingCutin>(root).is_none());
        let layout = app.world().get::<CutinLayout>(root).unwrap();
        assert_eq!(layout.source_size, Vec2::new(400.0, 300.0));
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn replacement_resets_a_dragged_cutin_to_canonical_placement() {
        let mut app = integration_app();
        enter_in_game(&mut app);

        write_cutin(&mut app, show("same", CutinPlacement::CenterWindow));
        app.update();
        let (_, first_handle, _) = single_pending(&mut app);
        insert_image(&mut app, &first_handle, fill_image(400, 300));
        app.update();

        let first = root_node(&mut app);
        assert_eq!(
            (px_or_zero(first.left), px_or_zero(first.top)),
            (200.0, 135.0)
        );

        // Real drag on the titlebar handle moves the cutin off its canonical center.
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(app.world())
            .next()
            .unwrap();
        let drag_handle = app
            .world_mut()
            .query_filtered::<Entity, With<CutinDragHandle>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .trigger(drag_event(drag_handle, window, Vec2::new(60.0, -40.0)));
        app.world_mut().flush();

        let dragged = root_node(&mut app);
        assert_eq!(
            (px_or_zero(dragged.left), px_or_zero(dragged.top)),
            (260.0, 95.0)
        );

        // Resend the same image; finalizing the replacement re-centers it.
        write_cutin(&mut app, show("same", CutinPlacement::CenterWindow));
        app.update();
        let (_, second_handle, _) = single_pending(&mut app);
        insert_image(&mut app, &second_handle, fill_image(400, 300));
        app.update();

        let second = root_node(&mut app);
        assert_eq!(
            (px_or_zero(second.left), px_or_zero(second.top)),
            (200.0, 135.0)
        );
    }

    #[test]
    fn drag_out_of_bounds_then_layout_clamps() {
        let mut app = integration_app();
        enter_in_game(&mut app);

        write_cutin(&mut app, show("event_illust", CutinPlacement::CenterWindow));
        app.update();
        let (_, handle, _) = single_pending(&mut app);
        insert_image(&mut app, &handle, fill_image(400, 300));
        app.update();

        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(app.world())
            .next()
            .unwrap();
        let drag_handle = app
            .world_mut()
            .query_filtered::<Entity, With<CutinDragHandle>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().trigger(drag_event(
            drag_handle,
            window,
            Vec2::new(-10000.0, 20000.0),
        ));
        app.world_mut().flush();

        app.update();
        let node = root_node(&mut app);
        assert!(px_or_zero(node.left) >= 0.0);
        assert!(px_or_zero(node.top) >= 0.0);
        assert!(px_or_zero(node.left) + px_or_zero(node.width) <= 800.0 + 1e-3);
        assert!(px_or_zero(node.top) + px_or_zero(node.height) <= 600.0 + 1e-3);
    }

    #[test]
    fn invalid_image_dimensions_despawn_the_root() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        write_cutin(&mut app, show("event_illust", CutinPlacement::BottomLeft));
        app.update();
        let (entity, handle, _) = single_pending(&mut app);

        insert_image(&mut app, &handle, zero_image());
        app.update();

        assert!(app.world().get_entity(entity).is_err());
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn load_failure_despawns_the_pending_root() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        write_cutin(&mut app, show("missing", CutinPlacement::BottomLeft));
        app.update();
        assert_eq!(root_count(&mut app), 1);

        for _ in 0..100 {
            app.update();
            if root_count(&mut app) == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn old_handle_completing_after_replacement_does_not_resurrect() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        write_cutin(&mut app, show("first", CutinPlacement::BottomLeft));
        app.update();
        let (first_root, first_handle, _) = single_pending(&mut app);

        write_cutin(&mut app, show("second", CutinPlacement::BottomLeft));
        app.update();
        let (second_root, second_handle, _) = single_pending(&mut app);

        assert!(app.world().get_entity(first_root).is_err());
        assert_ne!(first_handle, second_handle);

        insert_image(&mut app, &first_handle, fill_image(400, 300));
        insert_image(&mut app, &second_handle, fill_image(400, 300));
        app.update();

        assert_eq!(root_count(&mut app), 1);
        assert!(app.world().get_entity(second_root).is_ok());
        assert!(app.world().get_entity(first_root).is_err());
        assert_eq!(pending_count(&mut app), 0);
        assert!(app.world().get::<CutinLayout>(second_root).is_some());
    }

    #[test]
    fn leaving_in_game_despawns_the_root() {
        let mut app = lifecycle_app();
        enter_in_game(&mut app);
        app.world_mut().spawn((
            CutinRoot {
                placement: CutinPlacement::BottomLeft,
                dismissed: false,
            },
            DespawnOnExit(GameState::InGame),
        ));
        assert_eq!(root_count(&mut app), 1);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Bootstrapping);
        app.update();

        assert_eq!(root_count(&mut app), 0);
    }

    // ---- Scenes ------------------------------------------------------------

    fn scene_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    fn spawn_scene(scene: impl Scene) -> (App, Entity) {
        let mut app = scene_app();
        let root = app.world_mut().spawn_scene(scene).unwrap().id();
        (app, root)
    }

    #[test]
    fn static_scene_marks_root_and_image_pick_through() {
        let (mut app, root) = spawn_scene(static_cutin("portrait".to_string()));
        let world = app.world_mut();

        let root_pickable = world.get::<Pickable>(root).unwrap();
        assert!(!root_pickable.should_block_lower);
        assert!(!root_pickable.is_hoverable);

        let mut images = world.query_filtered::<Entity, With<CutinImage>>();
        assert_eq!(images.iter(world).count(), 1);
        let image = images.iter(world).next().unwrap();
        let image_pickable = world.get::<Pickable>(image).unwrap();
        assert!(!image_pickable.should_block_lower);
        assert!(!image_pickable.is_hoverable);
    }

    fn assert_hidden_below_overlays(scene: impl Scene) {
        let (mut app, root) = spawn_scene(scene);
        let world = app.world_mut();
        assert_eq!(world.get::<GlobalZIndex>(root).unwrap().0, CUTIN_Z);
        assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Hidden);
    }

    #[test]
    fn every_scene_root_hides_and_sits_below_overlays() {
        assert_hidden_below_overlays(static_cutin("portrait".to_string()));
        assert_hidden_below_overlays(windowed_cutin("portrait".to_string()));
        assert_hidden_below_overlays(chromeless_cutin("portrait".to_string()));
    }

    #[test]
    fn spawn_hidden_root_adds_lifecycle_markers() {
        let mut app = scene_app();
        app.add_systems(Update, move |mut commands: Commands| {
            spawn_hidden_root(
                &mut commands,
                "ro://data/texture/유저인터페이스/illust/portrait.bmp".to_string(),
                Handle::default(),
                CutinPlacement::BottomLeft,
            );
        });
        app.update();

        let world = app.world_mut();
        let mut roots = world.query_filtered::<Entity, With<CutinRoot>>();
        assert_eq!(roots.iter(world).count(), 1);
        let root = roots.iter(world).next().unwrap();
        assert_eq!(
            world.get::<DespawnOnExit<GameState>>(root).unwrap().0,
            GameState::InGame
        );
        assert!(world.get::<PendingCutin>(root).is_some());
        // A cutin is a top-level overlay, independent of the HUD and NPC dialog.
        assert!(world.get::<ChildOf>(root).is_none());
    }

    #[test]
    fn windowed_scene_has_draggable_titlebar_and_no_close_control() {
        let (mut app, _) = spawn_scene(windowed_cutin("portrait".to_string()));
        let world = app.world_mut();

        assert_eq!(
            world
                .query_filtered::<(), With<CutinTitlebar>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query_filtered::<(), With<CutinDragHandle>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world.query::<&FeathersButton>().iter(world).count(),
            0,
            "mode 3 must not expose a close button"
        );
        assert_eq!(
            world
                .query_filtered::<(), With<CutinClose>>()
                .iter(world)
                .count(),
            0,
            "mode 3 must not expose a close control"
        );
    }

    #[test]
    fn chromeless_scene_has_draggable_image_and_higher_close_control() {
        let (mut app, _) = spawn_scene(chromeless_cutin("portrait".to_string()));
        let world = app.world_mut();

        assert_eq!(
            world
                .query_filtered::<(), With<CutinImage>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query_filtered::<(), With<CutinDragHandle>>()
                .iter(world)
                .count(),
            1
        );
        let mut buttons = world.query::<(Entity, &FeathersButton)>();
        assert_eq!(buttons.iter(world).count(), 1);
        let (button, _) = buttons.iter(world).next().unwrap();
        assert_eq!(world.get::<ZIndex>(button).unwrap().0, 1);
        assert!(world.get::<CutinClose>(button).is_some());
    }

    // ---- Layout ------------------------------------------------------------

    fn layout_app(placement: CutinPlacement) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(UiScale(1.0));
        app.add_systems(Update, layout_cutin);
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..default()
            },
            PrimaryWindow,
        ));

        let root = app
            .world_mut()
            .spawn((
                CutinRoot {
                    placement,
                    dismissed: false,
                },
                CutinLayout::new(Vec2::new(400.0, 300.0)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    ..default()
                },
                Visibility::Hidden,
            ))
            .id();
        app.world_mut().spawn((
            CutinImage,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                ..default()
            },
            ChildOf(root),
        ));
        if placement == CutinPlacement::CenterWindow {
            app.world_mut().spawn((
                CutinTitlebar,
                CutinDragHandle,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(0.0),
                    height: Val::Px(TITLEBAR_HEIGHT),
                    ..default()
                },
                ChildOf(root),
            ));
        }
        app
    }

    fn root_node(app: &mut App) -> Node {
        app.world_mut()
            .query_filtered::<&Node, With<CutinRoot>>()
            .single(app.world())
            .unwrap()
            .clone()
    }

    fn set_window(app: &mut App, width: u32, height: u32) {
        let mut query = app.world_mut().query::<&mut Window>();
        let mut window = query.single_mut(app.world_mut()).unwrap();
        window.resolution.set(width as f32, height as f32);
    }

    #[test]
    fn layout_anchors_static_modes_and_reanchors_on_resize() {
        for (placement, expected_x) in [
            (CutinPlacement::BottomLeft, 0.0),
            (CutinPlacement::BottomCenter, 200.0),
            (CutinPlacement::BottomRight, 400.0),
        ] {
            let mut app = layout_app(placement);
            app.update();
            let node = root_node(&mut app);
            assert!((px_or_zero(node.left) - expected_x).abs() < 1e-4);
            assert!((px_or_zero(node.top) - 300.0).abs() < 1e-4);
            assert_eq!(px_or_zero(node.width), 400.0);
            assert_eq!(px_or_zero(node.height), 300.0);

            // Static modes recompute their bottom anchor when the viewport grows.
            set_window(&mut app, 1200, 900);
            app.update();
            let node = root_node(&mut app);
            let expected = if placement == CutinPlacement::BottomCenter {
                400.0
            } else if placement == CutinPlacement::BottomRight {
                800.0
            } else {
                0.0
            };
            assert!((px_or_zero(node.left) - expected).abs() < 1e-4);
            assert!((px_or_zero(node.top) - 600.0).abs() < 1e-4);
        }
    }

    #[test]
    fn layout_centers_windowed_mode_and_scales_titlebar() {
        let mut app = layout_app(CutinPlacement::CenterWindow);
        app.update();

        let root = root_node(&mut app);
        assert!((px_or_zero(root.left) - 200.0).abs() < 1e-4);
        assert!((px_or_zero(root.top) - 135.0).abs() < 1e-4);
        assert_eq!(px_or_zero(root.width), 400.0);
        assert_eq!(px_or_zero(root.height), 330.0);

        let mut images = app.world_mut().query_filtered::<&Node, With<CutinImage>>();
        let image = images.iter(app.world()).next().unwrap();
        assert_eq!(px_or_zero(image.top), TITLEBAR_HEIGHT);
        assert_eq!(px_or_zero(image.width), 400.0);
        assert_eq!(px_or_zero(image.height), 300.0);

        let mut titlebars = app
            .world_mut()
            .query_filtered::<&Node, With<CutinTitlebar>>();
        let titlebar = titlebars.iter(app.world()).next().unwrap();
        assert_eq!(px_or_zero(titlebar.width), 400.0);
        assert_eq!(px_or_zero(titlebar.height), TITLEBAR_HEIGHT);
    }

    #[test]
    fn layout_retains_centered_position_on_growth_and_clamps_on_shrink() {
        let mut app = layout_app(CutinPlacement::CenterWindow);
        app.update();
        let first = root_node(&mut app);

        // Growth keeps the centered cutin where the user left it.
        set_window(&mut app, 1200, 900);
        app.update();
        let grown = root_node(&mut app);
        assert_eq!(px_or_zero(grown.left), px_or_zero(first.left));
        assert_eq!(px_or_zero(grown.top), px_or_zero(first.top));

        // Shrinking clamps it back into view rather than stranding it off-screen.
        set_window(&mut app, 300, 200);
        app.update();
        let shrunk = root_node(&mut app);
        assert!(px_or_zero(shrunk.left) >= 0.0);
        assert!(px_or_zero(shrunk.top) >= 0.0);
        assert!(px_or_zero(shrunk.left) + px_or_zero(shrunk.width) <= 300.0 + 1e-3);
        assert!(px_or_zero(shrunk.top) + px_or_zero(shrunk.height) <= 200.0 + 1e-3);
    }

    #[test]
    fn mode_four_close_presentation_fits_tiny_viewport() {
        let mut app = ui_pipeline_app();
        enter_in_game(&mut app);
        set_ui_viewport(&mut app, 10, 10);

        write_cutin(
            &mut app,
            show("event_illust", CutinPlacement::CenterChromeless),
        );
        app.update();
        let (root, handle, _) = single_pending(&mut app);
        insert_image(&mut app, &handle, fill_image(400, 300));
        app.update();

        let root_bounds = node_bounds(&app, root);
        assert!(root_bounds.min.x >= -1e-3);
        assert!(root_bounds.min.y >= -1e-3);
        assert!(root_bounds.max.x <= 10.0 + 1e-3);
        assert!(root_bounds.max.y <= 10.0 + 1e-3);

        // Root clipping keeps Feathers focus-outline visuals from escaping.
        let root_node = app.world().get::<Node>(root).unwrap();
        assert!(!root_node.overflow.is_visible());

        // The actual chromeless-scene close control: a Feathers button carrying
        // the focus-indicator and tab-index markers, with the glyph as a child.
        let close = app
            .world_mut()
            .query_filtered::<Entity, With<CutinClose>>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!(app.world().get::<FeathersButton>(close).is_some());
        assert!(
            app.world()
                .get::<bevy_feathers::focus::FocusIndicator>(close)
                .is_some()
        );
        assert!(
            app.world()
                .get::<bevy::input_focus::tab_navigation::TabIndex>(close)
                .is_some()
        );

        let glyph = app
            .world_mut()
            .query_filtered::<Entity, With<CutinCloseGlyph>>()
            .iter(app.world())
            .next()
            .unwrap();
        let close_children = app.world().get::<Children>(close).unwrap();
        assert!(close_children.contains(&glyph));

        // The complete laid-out presentation — Feathers padding included in the
        // button box, its pickable bounds, and the caption glyph — fits the root.
        assert_rect_contains(root_bounds, node_bounds(&app, close));
        assert_rect_contains(root_bounds, node_bounds(&app, glyph));
    }

    // ---- Observers ---------------------------------------------------------

    fn drag_event(target: Entity, window: Entity, delta: Vec2) -> Pointer<Drag> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;

        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::ZERO,
                delta,
            },
            target,
        )
    }

    fn drag_observer_app(ui_scale: f32) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(UiScale(ui_scale));
        let root = app
            .world_mut()
            .spawn((
                CutinRoot {
                    placement: CutinPlacement::CenterWindow,
                    dismissed: false,
                },
                Node {
                    left: Val::Px(100.0),
                    top: Val::Px(50.0),
                    ..default()
                },
            ))
            .observe(drag_cutin)
            .id();
        let window = app.world_mut().spawn(Window::default()).id();
        (app, root, window)
    }

    #[test]
    fn drag_observer_divides_delta_by_ui_scale() {
        let (mut app, root, window) = drag_observer_app(2.0);
        let handle = app.world_mut().spawn((CutinDragHandle, ChildOf(root))).id();

        app.world_mut()
            .trigger(drag_event(handle, window, Vec2::new(20.0, 10.0)));
        app.world_mut().flush();

        let node = app.world().get::<Node>(root).unwrap();
        assert_eq!(px_or_zero(node.left), 110.0);
        assert_eq!(px_or_zero(node.top), 55.0);
    }

    #[test]
    fn drag_observer_rejects_bubbled_non_handle_targets() {
        let (mut app, root, window) = drag_observer_app(2.0);
        let close = app.world_mut().spawn(ChildOf(root)).id();

        app.world_mut()
            .trigger(drag_event(close, window, Vec2::new(20.0, 10.0)));
        app.world_mut().flush();

        let node = app.world().get::<Node>(root).unwrap();
        assert_eq!(px_or_zero(node.left), 100.0);
        assert_eq!(px_or_zero(node.top), 50.0);
    }

    #[test]
    fn dismiss_observer_hides_and_marks_dismissed_without_despawning() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let root = app
            .world_mut()
            .spawn((
                CutinRoot {
                    placement: CutinPlacement::CenterChromeless,
                    dismissed: false,
                },
                Visibility::Inherited,
            ))
            .id();
        let button = app.world_mut().spawn_empty().observe(dismiss_cutin).id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        assert!(app.world().get_entity(root).is_ok());
        let (cutin_root, visibility) = app
            .world_mut()
            .query::<(&CutinRoot, &Visibility)>()
            .single(app.world())
            .unwrap();
        assert!(cutin_root.dismissed);
        assert_eq!(*visibility, Visibility::Hidden);
    }
}
