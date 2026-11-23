//! Pins live on disk per character, so they survive a relog.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use net_contract::state::ZoneSession;

use super::{QuestLog, QuestPins};

/// Whether this session's pins were read, and what the file held after the last write.
#[derive(Resource, Default)]
pub(super) struct PinsPersistState {
    loaded: bool,
    last_saved: QuestPins,
}

fn quest_pins_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LIFTHRASIR_QUEST_PINS_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .expect("a platform config directory")
        .join("lifthrasir")
        .join("quests")
}

fn quest_pins_path(char_id: u32) -> PathBuf {
    quest_pins_dir().join(format!("{char_id}.ron"))
}

/// A character with no file, or a file that cannot be read, starts with no pins.
fn read_pins(path: &Path) -> QuestPins {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return QuestPins::default(),
        Err(e) => {
            warn!("quest pins: failed to read {}: {e}", path.display());
            return QuestPins::default();
        }
    };
    ron::from_str(&text).unwrap_or_else(|e| {
        warn!("quest pins: ignoring {}: {e}", path.display());
        QuestPins::default()
    })
}

fn write_pins(path: &Path, pins: &QuestPins) {
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        error!("quest pins: failed to create directory: {e}");
        return;
    }
    match ron::to_string(pins) {
        Ok(text) => {
            if let Err(e) = std::fs::write(path, text) {
                error!("quest pins: failed to write {}: {e}", path.display());
            }
        }
        Err(e) => error!("quest pins: failed to serialize: {e}"),
    }
}

/// Reads the character's pins once per session, as soon as the session knows the character.
///
/// The server's list may already be in the log, in which case the file's pins are
/// checked against it here; otherwise `replay_quests` prunes them when it arrives.
/// `last_saved` keeps the unpruned set so [`persist_quest_pins`] rewrites the file
/// without the gone ones.
pub(super) fn load_quest_pins(
    mut pins: ResMut<QuestPins>,
    mut persist: ResMut<PinsPersistState>,
    session: Res<ZoneSession>,
    log: Res<QuestLog>,
) {
    if persist.loaded || session.char_id == 0 {
        return;
    }
    *pins = read_pins(&quest_pins_path(session.char_id));
    persist.last_saved = pins.clone();
    pins.prune(&log);
    persist.loaded = true;
}

pub(super) fn persist_quest_pins(
    pins: Res<QuestPins>,
    mut persist: ResMut<PinsPersistState>,
    session: Res<ZoneSession>,
) {
    if !persist.loaded || session.char_id == 0 || *pins == persist.last_saved {
        return;
    }
    write_pins(&quest_pins_path(session.char_id), &pins);
    persist.last_saved = pins.clone();
}

/// Saves on the way out of the game and lets the next session read its own file.
pub(super) fn save_quest_pins_on_exit(
    pins: Res<QuestPins>,
    mut persist: ResMut<PinsPersistState>,
    session: Res<ZoneSession>,
) {
    if persist.loaded && session.char_id != 0 {
        write_pins(&quest_pins_path(session.char_id), &pins);
    }
    *persist = PinsPersistState::default();
}
