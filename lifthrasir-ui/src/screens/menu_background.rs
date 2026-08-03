use bevy::{prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef};
use game_engine::core::state::GameState;

/// Loaded through the `ro://` composite source. Paths are joined onto the data
/// folder root (`assets/data`), so the bare filename maps to `assets/data/main_bg.png`.
const BACKGROUND_IMAGE: &str = "ro://main_bg.png";
const BLUR_SHADER: &str = "ro://shaders/menu_background_blur.wgsl";

pub struct MenuBackgroundPlugin;

impl Plugin for MenuBackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<BlurredBackgroundMaterial>::default());
        app.add_systems(Startup, spawn_menu_background);
        app.add_systems(Update, toggle_on_transition);
    }
}

/// Full-screen images behind the menu screens. The menu roots have transparent
/// backgrounds so one of these shows through.
#[derive(Component)]
struct SharpMenuBackground;

#[derive(Component)]
struct BlurredMenuBackground;

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct BlurredBackgroundMaterial {
    #[texture(0)]
    #[sampler(1)]
    image: Handle<Image>,
}

impl UiMaterial for BlurredBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        BLUR_SHADER.into()
    }
}

fn background_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        ..default()
    }
}

fn spawn_menu_background(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<BlurredBackgroundMaterial>>,
) {
    let image = asset_server.load(BACKGROUND_IMAGE);
    commands.spawn((
        background_node(),
        ImageNode::new(image.clone()),
        GlobalZIndex(i32::MIN),
        Visibility::Hidden,
        Pickable::IGNORE,
        SharpMenuBackground,
    ));
    commands.spawn((
        background_node(),
        MaterialNode(materials.add(BlurredBackgroundMaterial { image })),
        GlobalZIndex(i32::MIN),
        Visibility::Hidden,
        Pickable::IGNORE,
        BlurredMenuBackground,
    ));
}

fn toggle_on_transition(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    mut sharp: Single<&mut Visibility, (With<SharpMenuBackground>, Without<BlurredMenuBackground>)>,
    mut blurred: Single<
        &mut Visibility,
        (With<BlurredMenuBackground>, Without<SharpMenuBackground>),
    >,
) {
    let Some(entered) = transitions
        .read()
        .last()
        .and_then(|event| event.entered.as_ref())
    else {
        return;
    };
    let (sharp_visibility, blurred_visibility) = match entered {
        GameState::Login | GameState::ServerSelection => (Visibility::Visible, Visibility::Hidden),
        GameState::CharacterSelection | GameState::CharacterCreation => {
            (Visibility::Hidden, Visibility::Visible)
        }
        _ => (Visibility::Hidden, Visibility::Hidden),
    };
    **sharp = sharp_visibility;
    **blurred = blurred_visibility;
}
