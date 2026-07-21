use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, auto_init_resource};

use net_contract::state::ZoneSession;

use crate::app::zone_domain_plugin::ZoneDomainAutoPlugin;
use crate::core::state::GameState;

use super::model::Hotbar;

#[derive(Resource, Default)]
#[auto_init_resource(plugin = ZoneDomainAutoPlugin)]
struct HotbarPersistState {
    loaded: bool,
    last_saved: Hotbar,
}

fn hotbar_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LIFTHRASIR_HOTBAR_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .expect("a platform config directory")
        .join("lifthrasir")
        .join("hotbar")
}

fn hotbar_path(char_id: u32) -> PathBuf {
    hotbar_dir().join(format!("{char_id}.ron"))
}

fn read_hotbar(path: &Path) -> Hotbar {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Hotbar::default();
    };
    ron::from_str(&text).unwrap_or_default()
}

fn write_hotbar(path: &Path, hotbar: &Hotbar) {
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent) {
            error!("hotbar: failed to create directory: {e}");
            return;
        }
    match ron::to_string(hotbar) {
        Ok(text) => {
            if let Err(e) = std::fs::write(path, text) {
                error!("hotbar: failed to write file: {e}");
            }
        }
        Err(e) => error!("hotbar: failed to serialize: {e}"),
    }
}

#[auto_add_system(
    plugin = ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
fn load_hotbar(
    mut hotbar: ResMut<Hotbar>,
    mut persist: ResMut<HotbarPersistState>,
    session: Res<ZoneSession>,
) {
    if persist.loaded || session.char_id == 0 {
        return;
    }
    let bar = read_hotbar(&hotbar_path(session.char_id));
    persist.last_saved = bar.clone();
    *hotbar = bar;
    persist.loaded = true;
}

#[auto_add_system(
    plugin = ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
fn persist_hotbar(
    hotbar: Res<Hotbar>,
    mut persist: ResMut<HotbarPersistState>,
    session: Res<ZoneSession>,
) {
    if !persist.loaded || session.char_id == 0 || *hotbar == persist.last_saved {
        return;
    }
    write_hotbar(&hotbar_path(session.char_id), &hotbar);
    persist.last_saved = hotbar.clone();
}

#[auto_add_system(
    plugin = ZoneDomainAutoPlugin,
    schedule = OnExit(GameState::InGame)
)]
fn reset_on_exit(
    hotbar: Res<Hotbar>,
    mut persist: ResMut<HotbarPersistState>,
    session: Res<ZoneSession>,
) {
    if persist.loaded && session.char_id != 0 {
        write_hotbar(&hotbar_path(session.char_id), &hotbar);
    }
    persist.loaded = false;
    persist.last_saved = Hotbar::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::hotbar::model::HotbarSlot;

    fn make_zone(char_id: u32) -> ZoneSession {
        ZoneSession {
            char_id,
            ..Default::default()
        }
    }

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("lifthrasir_hotbar_test")
            .join(name)
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Redirects `hotbar_path` at a unique temp dir for the duration of `f`,
    /// serialized so the process-global env var is never shared across tests.
    fn with_hotbar_dir<T>(name: &str, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tmp(name);
        let _ = std::fs::remove_dir_all(&dir);
        // FIXME: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("LIFTHRASIR_HOTBAR_DIR", &dir) };
        let result = f();
        // FIXME: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("LIFTHRASIR_HOTBAR_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn persist_and_load_round_trips() {
        let path = tmp("round_trip.ron");
        let _ = std::fs::remove_file(&path);

        let mut bar = Hotbar::default();
        bar.assign(0, HotbarSlot::Skill(10));
        bar.assign(5, HotbarSlot::Item(200));
        bar.assign(11, HotbarSlot::Skill(999));

        write_hotbar(&path, &bar);
        let restored = read_hotbar(&path);
        assert_eq!(bar, restored);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_returns_default() {
        let path = tmp("does_not_exist.ron");
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_hotbar(&path), Hotbar::default());
    }

    #[test]
    fn corrupt_ron_returns_default() {
        let path = tmp("corrupt.ron");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{{{{ not valid ron").unwrap();
        assert_eq!(read_hotbar(&path), Hotbar::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn write_creates_parent_directories() {
        let path = tmp("nested/subdir/char.ron");
        let _ = std::fs::remove_dir_all(tmp("nested"));

        write_hotbar(&path, &Hotbar::default());
        assert!(path.exists());

        let _ = std::fs::remove_dir_all(tmp("nested"));
    }

    #[test]
    fn load_system_reads_file_then_ignores_second_run() {
        with_hotbar_dir("load_once", || {
            const CHAR_ID: u32 = 9_988_001;
            let mut bar = Hotbar::default();
            bar.assign(1, HotbarSlot::Item(50));
            write_hotbar(&hotbar_path(CHAR_ID), &bar);

            let mut app = App::new();
            app.insert_resource(Hotbar::default())
                .insert_resource(HotbarPersistState::default())
                .insert_resource(make_zone(CHAR_ID))
                .add_systems(Update, load_hotbar);

            app.update();
            assert!(app.world().resource::<HotbarPersistState>().loaded);
            assert_eq!(
                app.world().resource::<Hotbar>().get(1),
                Some(HotbarSlot::Item(50))
            );

            app.world_mut()
                .resource_mut::<Hotbar>()
                .assign(1, HotbarSlot::Skill(99));
            app.update();
            assert_eq!(
                app.world().resource::<Hotbar>().get(1),
                Some(HotbarSlot::Skill(99))
            );
        });
    }

    #[test]
    fn persist_system_writes_file_and_load_system_restores_it() {
        with_hotbar_dir("persist_restore", || {
            const CHAR_ID: u32 = 9_988_002;
            let path = hotbar_path(CHAR_ID);

            let mut bar = Hotbar::default();
            bar.assign(0, HotbarSlot::Skill(10));
            bar.assign(7, HotbarSlot::Item(300));

            let mut app = App::new();
            app.insert_resource(bar.clone())
                .insert_resource(HotbarPersistState {
                    loaded: true,
                    last_saved: Hotbar::default(),
                })
                .insert_resource(make_zone(CHAR_ID))
                .add_systems(Update, persist_hotbar);
            app.update();

            assert!(path.exists());

            let mut app2 = App::new();
            app2.insert_resource(Hotbar::default())
                .insert_resource(HotbarPersistState::default())
                .insert_resource(make_zone(CHAR_ID))
                .add_systems(Update, load_hotbar);
            app2.update();

            assert_eq!(
                app2.world().resource::<Hotbar>().get(0),
                Some(HotbarSlot::Skill(10))
            );
            assert_eq!(
                app2.world().resource::<Hotbar>().get(7),
                Some(HotbarSlot::Item(300))
            );
        });
    }

    #[test]
    fn persist_system_skips_when_bar_unchanged() {
        with_hotbar_dir("persist_skip", || {
            const CHAR_ID: u32 = 9_988_003;
            let path = hotbar_path(CHAR_ID);

            let mut app = App::new();
            app.insert_resource(Hotbar::default())
                .insert_resource(HotbarPersistState {
                    loaded: true,
                    last_saved: Hotbar::default(),
                })
                .insert_resource(make_zone(CHAR_ID))
                .add_systems(Update, persist_hotbar);
            app.update();

            assert!(!path.exists());
        });
    }

    #[test]
    fn reset_on_exit_clears_loaded_flag() {
        let mut app = App::new();
        app.insert_resource(Hotbar::default())
            .insert_resource(HotbarPersistState {
                loaded: true,
                last_saved: Hotbar::default(),
            })
            .insert_resource(make_zone(0))
            .add_systems(Update, reset_on_exit);

        app.update();

        assert!(!app.world().resource::<HotbarPersistState>().loaded);
    }

    #[test]
    fn load_system_skips_when_char_id_is_zero() {
        let mut app = App::new();
        app.insert_resource(Hotbar::default())
            .insert_resource(HotbarPersistState::default())
            .insert_resource(make_zone(0))
            .add_systems(Update, load_hotbar);

        app.update();

        assert!(!app.world().resource::<HotbarPersistState>().loaded);
        assert_eq!(*app.world().resource::<Hotbar>(), Hotbar::default());
    }
}
