use std::collections::{HashMap, HashSet, VecDeque};

use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    tasks::{poll_once, IoTaskPool, Task},
};
use game_engine::domain::guild::GuildState;
use net_contract::{
    commands::{GuildEmblemFetchRequested, GuildEmblemUploadRequested},
    events::{GuildIngress, GuildIngressPayload, ZoneDisconnected},
    state::ZoneSessionGeneration,
};

use super::{
    GuildHeaderEmblemFallback, GuildHeaderEmblemImage, GuildUi, GuildWindowRoot,
    PendingGuildMutation,
};

const MAX_EMBLEM_BYTES: usize = 102_400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct EmblemKey {
    pub guild_id: u32,
    pub emblem_id: u32,
}

impl EmblemKey {
    pub(crate) fn new(guild_id: u32, emblem_id: u32) -> Option<Self> {
        (guild_id != 0 && emblem_id != 0).then_some(Self {
            guild_id,
            emblem_id,
        })
    }
}

struct PickerTask {
    generation: ZoneSessionGeneration,
    form_generation: u64,
    task: Task<Option<Vec<u8>>>,
}

#[derive(Resource, Default)]
pub(crate) struct GuildEmblemImages {
    generation: ZoneSessionGeneration,
    form_generation: u64,
    picker: Option<PickerTask>,
    cache: HashMap<EmblemKey, Handle<Image>>,
    failed: HashSet<EmblemKey>,
    queued: VecDeque<EmblemKey>,
    in_flight: Option<EmblemKey>,
    preview: Option<Handle<Image>>,
}

impl GuildEmblemImages {
    pub(crate) fn cached(&self, key: EmblemKey) -> Option<Handle<Image>> {
        self.cache.get(&key).cloned()
    }

    pub(crate) fn request(&mut self, key: EmblemKey) {
        if self.cache.contains_key(&key)
            || self.failed.contains(&key)
            || self.in_flight == Some(key)
            || self.queued.contains(&key)
        {
            return;
        }
        self.queued.push_back(key);
    }

    #[cfg(test)]
    pub(crate) fn has_queued(&self, key: EmblemKey) -> bool {
        self.queued.contains(&key)
    }

    #[cfg(test)]
    pub(crate) fn insert_cached_for_test(&mut self, key: EmblemKey, image: Handle<Image>) {
        self.cache.insert(key, image);
    }

    #[cfg(test)]
    pub(crate) fn remove_cached_for_test(&mut self, key: EmblemKey) {
        self.cache.remove(&key);
    }

    fn clear(&mut self, images: &mut Assets<Image>) {
        for handle in self.cache.values() {
            images.remove(handle);
        }
        if let Some(handle) = self.preview.take() {
            images.remove(&handle);
        }
        self.cache.clear();
        self.failed.clear();
        self.queued.clear();
        self.in_flight = None;
        self.picker = None;
        self.form_generation = self.form_generation.wrapping_add(1);
    }

    fn invalidate_form(&mut self) {
        self.form_generation = self.form_generation.wrapping_add(1);
    }

    pub(crate) fn discard_preview(&mut self, images: &mut Assets<Image>) {
        if let Some(handle) = self.preview.take() {
            images.remove(&handle);
        }
    }
}

fn can_upload_emblem(guild: &GuildState, session: &net_contract::state::ZoneSession) -> bool {
    session.char_id != 0
        && guild.is_master(session.char_id)
        && guild.member(session.char_id).is_some()
}

fn completed_picker_bytes(
    result: Option<Vec<u8>>,
    picker_generation: ZoneSessionGeneration,
    picker_form_generation: u64,
    generation: ZoneSessionGeneration,
    form_generation: u64,
    authorized: bool,
    pending: bool,
) -> Result<Option<Vec<u8>>, &'static str> {
    if picker_generation != generation || picker_form_generation != form_generation || !authorized {
        return Ok(None);
    }
    let Some(data) = result else {
        return Ok(None);
    };
    if pending {
        return Err("A guild action is already pending.");
    }
    Ok(Some(data))
}

pub(crate) fn on_select_emblem(
    _: On<bevy::ui_widgets::Activate>,
    guild: Res<GuildState>,
    session: Res<net_contract::state::ZoneSession>,
    generation: Res<ZoneSessionGeneration>,
    mut images: ResMut<GuildEmblemImages>,
) {
    if !can_upload_emblem(&guild, &session) || images.picker.is_some() {
        return;
    }
    let form_generation = images.form_generation;
    let task = IoTaskPool::get().spawn(async move {
        let file = rfd::AsyncFileDialog::new()
            .add_filter("Bitmap", &["bmp"])
            .pick_file()
            .await?;
        Some(file.read().await)
    });
    images.picker = Some(PickerTask {
        generation: *generation,
        form_generation,
        task,
    });
}

pub(crate) fn poll_picker(
    generation: Res<ZoneSessionGeneration>,
    guild: Res<GuildState>,
    session: Res<net_contract::state::ZoneSession>,
    mut images: ResMut<GuildEmblemImages>,
    mut ui: ResMut<GuildUi>,
    mut assets: ResMut<Assets<Image>>,
    mut uploads: MessageWriter<GuildEmblemUploadRequested>,
) {
    let Some(picker) = images.picker.as_mut() else {
        return;
    };
    let Some(result) = bevy::tasks::block_on(poll_once(&mut picker.task)) else {
        return;
    };
    let picker = images.picker.take().expect("picker task was present");
    let data = match completed_picker_bytes(
        result,
        picker.generation,
        picker.form_generation,
        *generation,
        images.form_generation,
        can_upload_emblem(&guild, &session),
        ui.pending.is_some(),
    ) {
        Ok(Some(data)) => data,
        Ok(None) => return,
        Err(message) => {
            ui.feedback = Some(message.to_string());
            ui.feedback_is_error = true;
            return;
        }
    };
    let image = match decode_bmp(&data) {
        Ok(image) => image,
        Err(message) => {
            ui.feedback = Some(message.to_string());
            ui.feedback_is_error = true;
            return;
        }
    };
    if let Some(handle) = images.preview.replace(assets.add(image)) {
        assets.remove(&handle);
    }
    ui.pending = Some(PendingGuildMutation {
        action: "emblem_upload",
        generation: *generation,
    });
    ui.feedback = Some("Uploading guild emblem…".to_string());
    ui.feedback_is_error = false;
    uploads.write(GuildEmblemUploadRequested { data });
}

pub(crate) fn queue_current_guild_emblem(
    guild: Res<GuildState>,
    mut images: ResMut<GuildEmblemImages>,
) {
    let Some(info) = guild.info() else {
        return;
    };
    let Some(key) = EmblemKey::new(info.guild_id, info.emblem_id) else {
        return;
    };
    images.request(key);
}

pub(crate) fn receive_emblem_data(
    generation: Res<ZoneSessionGeneration>,
    mut ingress: MessageReader<GuildIngress>,
    mut images: ResMut<GuildEmblemImages>,
    mut assets: ResMut<Assets<Image>>,
) {
    for event in ingress.read() {
        if event.generation != *generation {
            continue;
        }
        match &event.payload {
            GuildIngressPayload::ActionResult(result)
                if !result.success && result.action == "emblem_request" =>
            {
                if let Some(key) = images.in_flight.take() {
                    images.failed.insert(key);
                }
            }
            GuildIngressPayload::EmblemData {
                guild_id,
                emblem_id,
                data,
            } => {
                let Some(key) = EmblemKey::new(*guild_id, *emblem_id) else {
                    continue;
                };
                if images.in_flight != Some(key) {
                    continue;
                }
                images.in_flight = None;
                match decode_bmp(data) {
                    Ok(image) => {
                        images.cache.insert(key, assets.add(image));
                    }
                    Err(error) => {
                        warn!(?key, %error, "dropping invalid guild emblem data");
                        images.failed.insert(key);
                    }
                }
            }
            GuildIngressPayload::EmblemChanged {
                guild_id,
                emblem_id,
            } if EmblemKey::new(*guild_id, *emblem_id).is_some() => {
                images.discard_preview(&mut assets);
            }
            _ => {}
        }
    }
}

pub(crate) fn send_next_fetch(
    mut images: ResMut<GuildEmblemImages>,
    mut fetches: MessageWriter<GuildEmblemFetchRequested>,
) {
    if images.in_flight.is_some() {
        return;
    }
    let Some(key) = images.queued.pop_front() else {
        return;
    };
    images.in_flight = Some(key);
    fetches.write(GuildEmblemFetchRequested {
        guild_id: key.guild_id,
        emblem_id: key.emblem_id,
    });
}

pub(crate) fn sync_header_emblem(
    guild: Res<GuildState>,
    images: Res<GuildEmblemImages>,
    mut header: Query<(&mut ImageNode, &mut Visibility), With<GuildHeaderEmblemImage>>,
    mut fallback: Query<
        &mut Visibility,
        (
            With<GuildHeaderEmblemFallback>,
            Without<GuildHeaderEmblemImage>,
        ),
    >,
) {
    let Ok((mut image, mut visibility)) = header.single_mut() else {
        return;
    };
    let Ok(mut fallback) = fallback.single_mut() else {
        return;
    };
    let handle = images.preview.clone().or_else(|| {
        guild
            .info()
            .and_then(|info| EmblemKey::new(info.guild_id, info.emblem_id))
            .and_then(|key| images.cached(key))
    });
    let Some(handle) = handle else {
        *visibility = Visibility::Hidden;
        *fallback = Visibility::Inherited;
        return;
    };
    image.image = handle;
    *visibility = Visibility::Inherited;
    *fallback = Visibility::Hidden;
}

pub(crate) fn invalidate_picker_when_hidden(
    roots: Query<&Visibility, (With<GuildWindowRoot>, Changed<Visibility>)>,
    mut images: ResMut<GuildEmblemImages>,
) {
    if roots
        .iter()
        .any(|visibility| *visibility == Visibility::Hidden)
    {
        images.invalidate_form();
    }
}

pub(crate) fn reset_emblems(
    generation: Res<ZoneSessionGeneration>,
    mut disconnected: MessageReader<ZoneDisconnected>,
    mut images: ResMut<GuildEmblemImages>,
    mut assets: ResMut<Assets<Image>>,
) {
    let disconnected = disconnected.read().count() != 0;
    if images.generation == *generation && !disconnected {
        return;
    }
    images.clear(&mut assets);
    images.generation = *generation;
}

pub(crate) fn clear_emblems_on_exit(
    mut images: ResMut<GuildEmblemImages>,
    mut assets: ResMut<Assets<Image>>,
) {
    images.clear(&mut assets);
}

fn decode_bmp(data: &[u8]) -> Result<Image, &'static str> {
    if data.len() > MAX_EMBLEM_BYTES {
        return Err("Guild emblems must be 100 KB or smaller.");
    }
    if !data.starts_with(b"BM") {
        return Err("Guild emblems must be BMP files.");
    }
    let image = Image::from_buffer(
        data,
        ImageType::Extension("bmp"),
        CompressedImageFormats::all(),
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .map_err(|_| "Guild emblem BMP data is corrupt or truncated.")?;
    let size = image.texture_descriptor.size;
    if size.width != 24 || size.height != 24 {
        return Err("Guild emblems must be exactly 24 by 24 pixels.");
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::{GuildActionResult, GuildErrorKind};

    fn fetch_app() -> App {
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<GuildEmblemFetchRequested>()
            .insert_resource(ZoneSessionGeneration(7))
            .init_resource::<GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default())
            .add_systems(Update, (send_next_fetch, receive_emblem_data).chain());
        app
    }

    fn bmp(width: i32, height: i32) -> Vec<u8> {
        let row_bytes = (width as usize * 3).div_ceil(4) * 4;
        let pixel_bytes = row_bytes * height.unsigned_abs() as usize;
        let size = 54 + pixel_bytes;
        let mut bytes = vec![0; size];
        bytes[..2].copy_from_slice(b"BM");
        bytes[2..6].copy_from_slice(&(size as u32).to_le_bytes());
        bytes[10..14].copy_from_slice(&54_u32.to_le_bytes());
        bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
        bytes[18..22].copy_from_slice(&width.to_le_bytes());
        bytes[22..26].copy_from_slice(&height.to_le_bytes());
        bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
        bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
        bytes[34..38].copy_from_slice(&(pixel_bytes as u32).to_le_bytes());
        bytes
    }

    #[test]
    fn validates_only_a_complete_24_pixel_bmp() {
        assert!(decode_bmp(&bmp(24, 24)).is_ok());
        assert!(decode_bmp(b"BM").is_err());
        assert!(decode_bmp(&bmp(23, 24)).is_err());
        assert!(decode_bmp(b"not-a-bmp").is_err());
    }

    #[test]
    fn rejects_data_over_the_exact_limit() {
        let mut data = bmp(24, 24);
        data.resize(MAX_EMBLEM_BYTES, 0);
        assert!(decode_bmp(&data).is_ok());
        data.resize(MAX_EMBLEM_BYTES + 1, 0);
        assert!(matches!(
            decode_bmp(&data),
            Err("Guild emblems must be 100 KB or smaller.")
        ));
    }

    #[test]
    fn picker_completion_preserves_exact_bytes_and_rejects_stale_or_unauthorized_results() {
        let data = bmp(24, 24);
        let completed = completed_picker_bytes(
            Some(data.clone()),
            ZoneSessionGeneration(7),
            3,
            ZoneSessionGeneration(7),
            3,
            true,
            false,
        )
        .unwrap();
        assert_eq!(completed, Some(data.clone()));
        assert_eq!(
            completed_picker_bytes(
                Some(data.clone()),
                ZoneSessionGeneration(7),
                3,
                ZoneSessionGeneration(7),
                4,
                true,
                false,
            )
            .unwrap(),
            None
        );
        assert_eq!(
            completed_picker_bytes(
                Some(data),
                ZoneSessionGeneration(7),
                3,
                ZoneSessionGeneration(7),
                3,
                false,
                false,
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn scheduled_close_invalidates_a_same_frame_picker_completion() {
        fn close(mut root: Query<&mut Visibility, With<GuildWindowRoot>>) {
            *root.single_mut().unwrap() = Visibility::Hidden;
        }

        let mut app = App::new();
        app.init_resource::<GuildEmblemImages>();
        app.world_mut()
            .spawn((GuildWindowRoot, Visibility::Visible));
        app.add_systems(Update, (close, invalidate_picker_when_hidden).chain());
        let form_generation = app.world().resource::<GuildEmblemImages>().form_generation;

        app.update();

        let current = app.world().resource::<GuildEmblemImages>().form_generation;
        assert_ne!(current, form_generation);
        assert_eq!(
            completed_picker_bytes(
                Some(bmp(24, 24)),
                ZoneSessionGeneration(7),
                form_generation,
                ZoneSessionGeneration(7),
                current,
                true,
                false,
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn queues_each_tuple_once_and_keeps_failures_session_scoped() {
        let key = EmblemKey::new(7, 3).unwrap();
        let mut images = GuildEmblemImages::default();
        images.request(key);
        images.request(key);
        assert_eq!(images.queued.len(), 1);
        images.in_flight = images.queued.pop_front();
        images.failed.insert(key);
        images.in_flight = None;
        images.request(key);
        assert!(images.queued.is_empty());
    }

    #[test]
    fn fetches_exact_tuple_and_ignores_stale_or_failed_responses() {
        let key = EmblemKey::new(7, 3).unwrap();
        let stale = EmblemKey::new(7, 4).unwrap();
        let mut app = fetch_app();
        app.world_mut()
            .resource_mut::<GuildEmblemImages>()
            .request(key);
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(7),
            payload: GuildIngressPayload::EmblemData {
                guild_id: stale.guild_id,
                emblem_id: stale.emblem_id,
                data: bmp(24, 24),
            },
        });

        app.update();

        let fetches = app
            .world()
            .resource::<Messages<GuildEmblemFetchRequested>>();
        let sent: Vec<_> = fetches.iter_current_update_messages().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].guild_id, key.guild_id);
        assert_eq!(sent[0].emblem_id, key.emblem_id);
        assert_eq!(
            app.world().resource::<GuildEmblemImages>().in_flight,
            Some(key)
        );

        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(7),
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "emblem_request".to_string(),
                success: false,
                error: GuildErrorKind::InvalidEmblem,
            }),
        });
        app.update();

        let images = app.world().resource::<GuildEmblemImages>();
        assert!(images.failed.contains(&key));
        assert!(images.in_flight.is_none());
        assert!(images.cached(key).is_none());
    }

    #[test]
    fn valid_response_caches_once_and_invalid_response_suppresses_retry() {
        let key = EmblemKey::new(7, 3).unwrap();
        let mut app = fetch_app();
        app.world_mut()
            .resource_mut::<GuildEmblemImages>()
            .request(key);
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(7),
            payload: GuildIngressPayload::EmblemData {
                guild_id: key.guild_id,
                emblem_id: key.emblem_id,
                data: bmp(24, 24),
            },
        });
        app.update();
        assert!(app
            .world()
            .resource::<GuildEmblemImages>()
            .cached(key)
            .is_some());

        let invalid = EmblemKey::new(7, 4).unwrap();
        app.world_mut()
            .resource_mut::<GuildEmblemImages>()
            .request(invalid);
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(7),
            payload: GuildIngressPayload::EmblemData {
                guild_id: invalid.guild_id,
                emblem_id: invalid.emblem_id,
                data: b"invalid".to_vec(),
            },
        });
        app.update();
        let mut images = app.world_mut().resource_mut::<GuildEmblemImages>();
        assert!(images.failed.contains(&invalid));
        images.request(invalid);
        assert!(!images.has_queued(invalid));
    }

    #[test]
    fn reset_drains_all_disconnects_without_a_later_second_clear() {
        let mut app = App::new();
        app.add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(7))
            .init_resource::<GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default())
            .add_systems(Update, reset_emblems);
        app.update();
        let after_generation_reset = app.world().resource::<GuildEmblemImages>().form_generation;
        app.world_mut().write_message(ZoneDisconnected {
            reason: "first".to_string(),
        });
        app.world_mut().write_message(ZoneDisconnected {
            reason: "second".to_string(),
        });

        app.update();
        let after_disconnect = app.world().resource::<GuildEmblemImages>().form_generation;
        app.update();

        assert_eq!(after_disconnect, after_generation_reset.wrapping_add(1));
        assert_eq!(
            app.world().resource::<GuildEmblemImages>().form_generation,
            after_disconnect
        );
    }

    #[test]
    fn rejected_upload_discards_preview_and_restores_header_fallback() {
        let generation = ZoneSessionGeneration(7);
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .insert_resource(generation)
            .insert_resource(GuildUi {
                pending: Some(PendingGuildMutation {
                    action: "emblem_upload",
                    generation,
                }),
                ..default()
            })
            .init_resource::<GuildState>()
            .init_resource::<GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default());
        let preview = {
            let mut assets = app.world_mut().resource_mut::<Assets<Image>>();
            assets.add(decode_bmp(&bmp(24, 24)).unwrap())
        };
        app.world_mut().resource_mut::<GuildEmblemImages>().preview = Some(preview.clone());
        app.world_mut().spawn((
            GuildHeaderEmblemImage,
            ImageNode::default(),
            Visibility::Inherited,
        ));
        app.world_mut()
            .spawn((GuildHeaderEmblemFallback, Visibility::Hidden));
        app.add_systems(
            Update,
            (super::super::apply_guild_results, sync_header_emblem).chain(),
        );
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "emblem_upload".to_string(),
                success: false,
                error: GuildErrorKind::InvalidEmblem,
            }),
        });

        app.update();

        assert!(app
            .world()
            .resource::<GuildEmblemImages>()
            .preview
            .is_none());
        assert!(app
            .world()
            .resource::<Assets<Image>>()
            .get(&preview)
            .is_none());
        let world = app.world_mut();
        assert_eq!(
            *world
                .query_filtered::<&Visibility, With<GuildHeaderEmblemImage>>()
                .single(world)
                .unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            *world
                .query_filtered::<&Visibility, With<GuildHeaderEmblemFallback>>()
                .single(world)
                .unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn session_clear_removes_preview_and_cached_assets() {
        let key = EmblemKey::new(7, 3).unwrap();
        let mut assets = Assets::default();
        let cached = assets.add(decode_bmp(&bmp(24, 24)).unwrap());
        let preview = assets.add(decode_bmp(&bmp(24, 24)).unwrap());
        let mut images = GuildEmblemImages::default();
        images.cache.insert(key, cached.clone());
        images.preview = Some(preview.clone());
        images.request(key);
        images.in_flight = Some(key);
        images.failed.insert(key);

        images.clear(&mut assets);

        assert!(assets.get(&cached).is_none());
        assert!(assets.get(&preview).is_none());
        assert!(images.cache.is_empty());
        assert!(images.queued.is_empty());
        assert!(images.in_flight.is_none());
        assert!(images.failed.is_empty());
    }
}
