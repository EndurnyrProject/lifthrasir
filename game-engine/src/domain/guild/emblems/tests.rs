use super::*;

#[test]
fn guild_plugin_provides_emblem_cache_without_ui() {
    let mut app = App::new();
    app.add_message::<GuildIngress>()
        .add_message::<ZoneDisconnected>()
        .insert_resource(ZoneSessionGeneration(7))
        .add_plugins(crate::domain::guild::GuildPlugin);
    app.update();
    assert!(app.world().contains_resource::<GuildEmblemImages>());
    let key = EmblemKey::new(7, 3).unwrap();
    app.world_mut()
        .resource_mut::<GuildEmblemImages>()
        .request(key);
    app.world_mut()
        .resource_mut::<GuildEmblemImages>()
        .request(key);
    app.update();
    assert_eq!(
        app.world()
            .resource::<Messages<GuildEmblemFetchRequested>>()
            .len(),
        1
    );
}

#[test]
fn session_clear_removes_cached_assets_and_pending_requests() {
    let key = EmblemKey::new(7, 3).unwrap();
    let mut assets = Assets::default();
    let cached = assets.add(decode_bmp(&bmp(24, 24)).unwrap());
    let mut images = GuildEmblemImages::default();
    images.cache.insert(key, cached.clone());
    images.request(key);
    images.in_flight = Some(key);
    images.failed.insert(key);
    images.clear(&mut assets);
    assert!(assets.get(&cached).is_none());
    assert!(images.cache.is_empty());
    assert!(images.queued.is_empty());
    assert!(images.in_flight.is_none());
    assert!(images.failed.is_empty());
}

use super::decode_emblem_bmp as decode_bmp;

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
    assert!(
        app.world()
            .resource::<GuildEmblemImages>()
            .cached(key)
            .is_some()
    );

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
fn reset_keeps_cached_emblems_for_the_same_zone_session() {
    let key = EmblemKey::new(7, 3).unwrap();
    let mut app = App::new();
    app.add_message::<ZoneDisconnected>()
        .insert_resource(ZoneSessionGeneration(7))
        .init_resource::<GuildEmblemImages>()
        .insert_resource(Assets::<Image>::default())
        .add_systems(Update, reset_emblems);
    app.update();
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(decode_bmp(&bmp(24, 24)).unwrap());
    app.world_mut()
        .resource_mut::<GuildEmblemImages>()
        .cache
        .insert(key, image);

    app.update();

    assert!(
        app.world()
            .resource::<GuildEmblemImages>()
            .cached(key)
            .is_some()
    );
}
