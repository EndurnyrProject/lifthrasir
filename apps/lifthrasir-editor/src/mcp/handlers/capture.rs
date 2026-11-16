//! Images for the agent: the editor camera's view, or a top-down orthographic
//! render of a GAT rectangle. A temporary offscreen camera renders them for a
//! few frames, so the human's viewport never changes; the PNG reply is sent
//! from the screenshot observer.

use crate::{
    EditorOnly,
    map::{layout::MapLayout, open::OpenMap, tools::gat::GAT_CELL_SIZE},
    mcp::{
        call::{AgentError, AgentOutput, AgentResult, Call, TopDownParams},
        coords::{cell_center, from_wire, in_bounds},
    },
    shell::camera::EditorCamera,
};
use bevy::{
    camera::{Exposure, RenderTarget, ScalingMode},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use lifthrasir_runtime::{rendering::billboard::EquipmentPreviewCamera, world::LifMapData};
use ro_formats::RoAltitude;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// The largest side of a top-down image, in pixels.
pub const MAX_PX: u32 = 2048;
/// Size of a viewport screenshot.
pub const SCREENSHOT_PX: UVec2 = UVec2::new(1280, 720);
/// Frames the new camera renders before its image is read back.
const WARMUP_FRAMES: u32 = 2;
/// Frames after which a capture that never arrived is abandoned.
const TIMEOUT_FRAMES: u32 = 300;
/// How far above the terrain the top-down camera sits, in world units.
const TOP_DOWN_HEIGHT: f32 = 5_000.0;

/// Where a top-down camera sits and what it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct TopDownFrame {
    pub transform: Transform,
    /// World units covered left to right (x) and top to bottom (z).
    pub width: f32,
    pub height: f32,
    /// Image size in pixels.
    pub px: UVec2,
    pub px_per_cell: u32,
}

/// Frames the GAT rectangle `min..=max` from straight above, north (higher
/// cell y) at the top of the image and higher x to the right.
pub fn top_down_frame(
    altitude: &RoAltitude,
    min: UVec2,
    max: UVec2,
) -> Result<TopDownFrame, AgentError> {
    let bounds = UVec2::new(altitude.width, altitude.height);
    let (min, max) = (in_bounds(min, bounds)?, in_bounds(max, bounds)?);
    if min.x > max.x || min.y > max.y {
        return Err(AgentError::Invalid("rect min exceeds max".into()));
    }
    let (low, high) = (cell_center(altitude, min)?, cell_center(altitude, max)?);
    let center = (low + high) / 2.0;
    let cells = max - min + UVec2::ONE;
    let px_per_cell = (MAX_PX / cells.max_element()).clamp(1, 32);
    let eye = Vec3::new(center.x, low.y.max(high.y) + TOP_DOWN_HEIGHT, center.z);
    Ok(TopDownFrame {
        transform: Transform::from_translation(eye).looking_at(eye - Vec3::Y, Vec3::NEG_Z),
        width: cells.x as f32 * GAT_CELL_SIZE,
        height: cells.y as f32 * GAT_CELL_SIZE,
        px: cells * px_per_cell,
        px_per_cell,
    })
}

/// A capture camera waiting to be read back.
#[derive(Component)]
pub struct PendingCapture {
    frames: u32,
    /// The screenshot request once spawned; despawned with a timed-out capture.
    screenshot: Option<Entity>,
    image: Handle<Image>,
    meta: Value,
    reply: Arc<Mutex<Option<oneshot::Sender<AgentResult>>>>,
}

/// Starts a capture; the reply is sent once the image is read back.
pub fn start(world: &mut World, call: Call, reply: oneshot::Sender<AgentResult>) {
    if let Err(error) = spawn_capture(world, call, reply) {
        error.send();
    }
}

/// A failed start that still owns the reply.
struct Refused(AgentError, oneshot::Sender<AgentResult>);

impl Refused {
    fn send(self) {
        let _ = self.1.send(Err(self.0));
    }
}

fn spawn_capture(
    world: &mut World,
    call: Call,
    reply: oneshot::Sender<AgentResult>,
) -> Result<(), Refused> {
    let plan = match plan(world, &call) {
        Ok(plan) => plan,
        Err(error) => return Err(Refused(error, reply)),
    };
    let (transform, projection, px, meta) = plan;
    let mut image = Image::new_target_texture(px.x, px.y, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = world.resource_mut::<Assets<Image>>().add(image);
    let editor = world
        .query_filtered::<(Option<&Exposure>, Option<&Tonemapping>), With<EditorCamera>>()
        .iter(world)
        .next()
        .map(|(exposure, tonemapping)| (exposure.copied(), tonemapping.copied()));
    let (exposure, tonemapping) = editor.unwrap_or_default();
    world.spawn((
        Name::new("mcp capture camera"),
        Camera3d::default(),
        Camera {
            order: -1,
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        exposure.unwrap_or_default(),
        tonemapping.unwrap_or_default(),
        projection,
        transform,
        // Not the world camera: the runtime's billboard and effect systems
        // find that one with `single()`.
        EquipmentPreviewCamera,
        EditorOnly,
        PendingCapture {
            frames: 0,
            screenshot: None,
            image,
            meta,
            reply: Arc::new(Mutex::new(Some(reply))),
        },
    ));
    Ok(())
}

type Plan = (Transform, Projection, UVec2, Value);

/// Camera placement, projection, image size and the reply's JSON for `call`.
fn plan(world: &mut World, call: &Call) -> Result<Plan, AgentError> {
    world
        .get_resource::<OpenMap>()
        .ok_or(AgentError::NoMapOpen)?;
    let root = world
        .get_resource::<MapLayout>()
        .ok_or(AgentError::NoMapOpen)?
        .root;
    match call {
        Call::Screenshot => {
            let (transform, projection) = world
                .query_filtered::<(&Transform, &Projection), With<EditorCamera>>()
                .iter(world)
                .next()
                .map(|(transform, projection)| (*transform, projection.clone()))
                .ok_or_else(|| AgentError::NotFound("the editor camera".into()))?;
            let meta = json!({ "size": [SCREENSHOT_PX.x, SCREENSHOT_PX.y] });
            Ok((transform, projection, SCREENSHOT_PX, meta))
        }
        Call::RenderTopDown(TopDownParams { min, max }) => {
            let altitude = &world
                .get::<LifMapData>(root)
                .ok_or(AgentError::NoMapOpen)?
                .altitude;
            let frame = top_down_frame(altitude, from_wire(*min), from_wire(*max))?;
            let projection = Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: frame.width,
                    height: frame.height,
                },
                far: TOP_DOWN_HEIGHT * 4.0,
                ..OrthographicProjection::default_3d()
            });
            let meta = json!({
                "rect": { "min": min, "max": max },
                "size": [frame.px.x, frame.px.y],
                "px_per_cell": frame.px_per_cell,
                "orientation": "top = +y (north), right = +x; pixel (0, 0) is cell [min.x, max.y]",
            });
            Ok((frame.transform, projection, frame.px, meta))
        }
        _ => Err(AgentError::Invalid(format!(
            "{} is not a capture",
            call.name()
        ))),
    }
}

/// Lets each capture camera render a few frames, then reads its image back;
/// abandons captures that never arrive.
pub fn tick_captures(
    mut commands: Commands,
    mut captures: Query<(Entity, &mut PendingCapture)>,
    mut images: ResMut<Assets<Image>>,
) {
    for (camera, mut capture) in &mut captures {
        capture.frames += 1;
        if capture.frames >= TIMEOUT_FRAMES {
            if let Some(reply) = capture.reply.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = reply.send(Err(AgentError::Invalid("the capture timed out".into())));
            }
            images.remove(&capture.image);
            if let Some(screenshot) = capture.screenshot {
                commands.entity(screenshot).try_despawn();
            }
            commands.entity(camera).despawn();
            continue;
        }
        if capture.screenshot.is_some() || capture.frames < WARMUP_FRAMES {
            continue;
        }
        let reply = capture.reply.clone();
        let meta = capture.meta.clone();
        let image = capture.image.clone();
        let screenshot = commands.spawn(Screenshot::image(image.clone())).id();
        capture.screenshot = Some(screenshot);
        commands.entity(screenshot).observe(
            move |captured: On<ScreenshotCaptured>,
                  mut commands: Commands,
                  mut images: ResMut<Assets<Image>>| {
                if let Some(reply) = reply.lock().ok().and_then(|mut slot| slot.take()) {
                    let _ = reply.send(png(&captured.image).map(|png| AgentOutput {
                        json: meta.clone(),
                        png: Some(png),
                    }));
                }
                images.remove(&image);
                commands.entity(camera).despawn();
            },
        );
    }
}

fn png(image: &Image) -> Result<Vec<u8>, AgentError> {
    let dynamic = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| AgentError::Invalid(format!("reading the capture: {error}")))?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    dynamic
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|error| AgentError::Invalid(format!("encoding the capture: {error}")))?;
    Ok(bytes.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_formats::gat::{GatCell, GatCellType};

    fn altitude(width: u32, height: u32) -> RoAltitude {
        RoAltitude {
            version: "1.2".into(),
            width,
            height,
            cells: vec![
                GatCell {
                    height: [0.0; 4],
                    cell_type: GatCellType::from(0),
                };
                (width * height) as usize
            ],
        }
    }

    #[test]
    fn top_down_frame_is_north_up() {
        let altitude = altitude(16, 16);
        let (min, max) = (UVec2::new(2, 3), UVec2::new(9, 12));
        let frame = top_down_frame(&altitude, min, max).unwrap();

        let view = frame.transform.to_matrix().inverse();
        let low = view.transform_point3(cell_center(&altitude, min).unwrap());
        let high = view.transform_point3(cell_center(&altitude, max).unwrap());
        assert!(high.y > low.y, "north is not up: {low} {high}");
        assert!(high.x > low.x, "east is not right: {low} {high}");
        assert!(
            low.z < 0.0 && high.z < 0.0,
            "the terrain is behind the camera"
        );
    }

    #[test]
    fn top_down_frame_clamps_pixels() {
        let one = top_down_frame(&altitude(4, 4), UVec2::ZERO, UVec2::ZERO).unwrap();
        assert_eq!(one.px_per_cell, 32);
        assert_eq!(one.px, UVec2::splat(32));

        let big = top_down_frame(&altitude(400, 400), UVec2::ZERO, UVec2::splat(399)).unwrap();
        assert_eq!(big.px_per_cell, 5);
        assert!(big.px.max_element() <= MAX_PX);
    }

    #[test]
    fn top_down_large_rect_fits_maximum_pixels() {
        let frame = top_down_frame(&altitude(1024, 1024), UVec2::ZERO, UVec2::splat(1023)).unwrap();
        assert!(frame.px.max_element() <= MAX_PX);
    }

    #[test]
    fn top_down_rect_off_map_is_out_of_bounds() {
        let result = top_down_frame(&altitude(4, 4), UVec2::ZERO, UVec2::new(4, 0));
        assert!(
            matches!(result, Err(AgentError::OutOfBounds(_))),
            "{result:?}"
        );
    }
}
