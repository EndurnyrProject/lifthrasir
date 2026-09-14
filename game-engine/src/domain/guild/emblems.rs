//! Downloaded guild emblems shared by world rendering and UI.

use super::plugin::GuildSessionGate;
use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
};
use net_contract::{
    commands::GuildEmblemFetchRequested,
    events::{GuildIngress, GuildIngressPayload, ZoneDisconnected},
    state::{ZoneSession, ZoneSessionGeneration},
};
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_EMBLEM_BYTES: usize = 102_400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmblemKey {
    pub guild_id: u32,
    pub emblem_id: u32,
}

impl EmblemKey {
    pub fn new(guild_id: u32, emblem_id: u32) -> Option<Self> {
        (guild_id != 0 && emblem_id != 0).then_some(Self {
            guild_id,
            emblem_id,
        })
    }
}

#[derive(Resource, Default)]
pub struct GuildEmblemImages {
    generation: ZoneSessionGeneration,
    char_id: u32,
    cache: HashMap<EmblemKey, Handle<Image>>,
    failed: HashSet<EmblemKey>,
    queued: VecDeque<EmblemKey>,
    in_flight: Option<EmblemKey>,
}

impl GuildEmblemImages {
    pub fn cached(&self, key: EmblemKey) -> Option<Handle<Image>> {
        self.cache.get(&key).cloned()
    }

    pub fn request(&mut self, key: EmblemKey) {
        if self.cache.contains_key(&key)
            || self.failed.contains(&key)
            || self.in_flight == Some(key)
            || self.queued.contains(&key)
        {
            return;
        }
        self.queued.push_back(key);
    }

    fn clear(&mut self, images: &mut Assets<Image>) {
        for handle in self.cache.values() {
            images.remove(handle);
        }
        self.cache.clear();
        self.failed.clear();
        self.queued.clear();
        self.in_flight = None;
    }
}

fn blocked(gate: Option<&GuildSessionGate>) -> bool {
    gate.is_some_and(|gate| gate.blocked)
}

pub(super) fn receive_emblem_data(
    generation: Res<ZoneSessionGeneration>,
    gate: Option<Res<GuildSessionGate>>,
    mut ingress: MessageReader<GuildIngress>,
    mut images: ResMut<GuildEmblemImages>,
    mut assets: ResMut<Assets<Image>>,
) {
    if blocked(gate.as_deref()) {
        ingress.clear();
        return;
    }
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
                match decode_emblem_bmp(data) {
                    Ok(image) => {
                        images.cache.insert(key, assets.add(image));
                    }
                    Err(error) => {
                        warn!(?key, %error, "dropping invalid guild emblem data");
                        images.failed.insert(key);
                    }
                }
            }
            _ => {}
        }
    }
}

pub(super) fn send_next_fetch(
    gate: Option<Res<GuildSessionGate>>,
    mut images: ResMut<GuildEmblemImages>,
    mut fetches: MessageWriter<GuildEmblemFetchRequested>,
) {
    if blocked(gate.as_deref()) || images.in_flight.is_some() {
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

pub(super) fn reset_emblems(
    generation: Res<ZoneSessionGeneration>,
    session: Option<Res<ZoneSession>>,
    mut disconnected: Option<MessageReader<ZoneDisconnected>>,
    mut images: ResMut<GuildEmblemImages>,
    mut assets: ResMut<Assets<Image>>,
) {
    let disconnected = disconnected.as_mut().is_some_and(|r| r.read().count() != 0);
    let char_id = session.as_deref().map_or(0, |s| s.char_id);
    if images.generation == *generation && images.char_id == char_id && !disconnected {
        return;
    }
    images.clear(&mut assets);
    images.generation = *generation;
    images.char_id = char_id;
}

pub(super) fn block_emblems(
    mut images: ResMut<GuildEmblemImages>,
    mut assets: ResMut<Assets<Image>>,
) {
    images.clear(&mut assets);
}

/// Validate and decode the protocol's bounded 24×24 BMP payload.
pub fn decode_emblem_bmp(data: &[u8]) -> Result<Image, &'static str> {
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

/// Minimal uncompressed 24-bit BMP of the given size, for tests across crates.
#[doc(hidden)]
pub fn test_emblem_bmp(width: i32, height: i32) -> Vec<u8> {
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

#[cfg(test)]
mod tests;
