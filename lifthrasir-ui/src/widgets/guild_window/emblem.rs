use super::{
    GuildHeaderEmblemFallback, GuildHeaderEmblemImage, GuildUi, GuildUiSession, GuildWindowRoot,
    PendingGuildMutation,
};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, poll_once},
};
use game_engine::domain::guild::{
    GuildState,
    emblems::{EmblemKey, GuildEmblemImages, decode_emblem as decode_bmp},
};
use net_contract::{
    commands::GuildEmblemUploadRequested,
    events::{GuildIngress, GuildIngressPayload, ZoneDisconnected},
    state::ZoneSessionGeneration,
};

struct PickerTask {
    generation: ZoneSessionGeneration,
    form_generation: u64,
    task: Task<Option<Vec<u8>>>,
}

#[derive(Resource, Default)]
pub(crate) struct GuildEmblemPreview {
    generation: ZoneSessionGeneration,
    form_generation: u64,
    picker: Option<PickerTask>,
    preview: Option<Handle<Image>>,
}

impl GuildEmblemPreview {
    fn clear(&mut self, images: &mut Assets<Image>) {
        self.discard_preview(images);
        self.picker = None;
        self.invalidate_form();
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
    mut images: ResMut<GuildEmblemPreview>,
) {
    if !can_upload_emblem(&guild, &session) || images.picker.is_some() {
        return;
    }
    let form_generation = images.form_generation;
    let task = IoTaskPool::get().spawn(async move {
        let file = rfd::AsyncFileDialog::new()
            .add_filter("Image", &["bmp", "png"])
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
    mut images: ResMut<GuildEmblemPreview>,
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

pub(crate) fn sync_header_emblem(
    guild: Res<GuildState>,
    images: Res<GuildEmblemImages>,
    preview: Res<GuildEmblemPreview>,
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
    let handle = preview.preview.clone().or_else(|| {
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
    mut images: ResMut<GuildEmblemPreview>,
) {
    if roots
        .iter()
        .any(|visibility| *visibility == Visibility::Hidden)
    {
        images.invalidate_form();
    }
}

pub(crate) fn reset_emblem_preview(
    generation: Res<ZoneSessionGeneration>,
    mut disconnected: Option<MessageReader<ZoneDisconnected>>,
    session: Option<Res<GuildUiSession>>,
    mut images: ResMut<GuildEmblemPreview>,
    mut assets: ResMut<Assets<Image>>,
) {
    let disconnected = disconnected
        .as_mut()
        .is_some_and(|reader| reader.read().count() != 0);
    let session_reset = session.as_deref().is_some_and(|session| session.reset);
    if images.generation == *generation && !disconnected && !session_reset {
        return;
    }
    images.clear(&mut assets);
    images.generation = *generation;
}

pub(crate) fn receive_emblem_changes(
    generation: Res<ZoneSessionGeneration>,
    session: Option<Res<GuildUiSession>>,
    mut ingress: MessageReader<GuildIngress>,
    mut preview: ResMut<GuildEmblemPreview>,
    mut images: ResMut<Assets<Image>>,
) {
    if session.as_deref().is_some_and(|s| s.blocked) {
        ingress.clear();
        return;
    }
    for event in ingress.read() {
        if event.generation != *generation {
            continue;
        }
        if let GuildIngressPayload::EmblemChanged {
            guild_id,
            emblem_id,
        } = event.payload
            && EmblemKey::new(guild_id, emblem_id).is_some()
        {
            preview.discard_preview(&mut images);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::guild::emblems::test_emblem_bmp as bmp;
    use net_contract::dto::{GuildActionResult, GuildErrorKind};

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
        app.init_resource::<GuildEmblemPreview>();
        app.world_mut()
            .spawn((GuildWindowRoot, Visibility::Visible));
        app.add_systems(Update, (close, invalidate_picker_when_hidden).chain());
        let form_generation = app.world().resource::<GuildEmblemPreview>().form_generation;

        app.update();

        let current = app.world().resource::<GuildEmblemPreview>().form_generation;
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
    fn reset_drains_all_disconnects_without_a_later_second_clear() {
        let mut app = App::new();
        app.add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(7))
            .init_resource::<GuildEmblemPreview>()
            .insert_resource(Assets::<Image>::default())
            .add_systems(Update, reset_emblem_preview);
        app.update();
        let after_generation_reset = app.world().resource::<GuildEmblemPreview>().form_generation;
        app.world_mut().write_message(ZoneDisconnected {
            reason: "first".to_string(),
        });
        app.world_mut().write_message(ZoneDisconnected {
            reason: "second".to_string(),
        });

        app.update();
        let after_disconnect = app.world().resource::<GuildEmblemPreview>().form_generation;
        app.update();

        assert_eq!(after_disconnect, after_generation_reset.wrapping_add(1));
        assert_eq!(
            app.world().resource::<GuildEmblemPreview>().form_generation,
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
            .init_resource::<GuildEmblemPreview>()
            .init_resource::<GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default());
        let preview = {
            let mut assets = app.world_mut().resource_mut::<Assets<Image>>();
            assets.add(decode_bmp(&bmp(24, 24)).unwrap())
        };
        app.world_mut().resource_mut::<GuildEmblemPreview>().preview = Some(preview.clone());
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

        assert!(
            app.world()
                .resource::<GuildEmblemPreview>()
                .preview
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&preview)
                .is_none()
        );
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
    fn session_clear_removes_preview_assets() {
        let mut assets = Assets::default();
        let preview = assets.add(decode_bmp(&bmp(24, 24)).unwrap());
        let mut images = GuildEmblemPreview {
            preview: Some(preview.clone()),
            ..default()
        };
        images.clear(&mut assets);
        assert!(assets.get(&preview).is_none());
        assert!(images.preview.is_none());
    }
}
