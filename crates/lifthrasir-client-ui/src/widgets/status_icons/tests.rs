use super::*;
use lifthrasir_assets::catalogs::skill::SkillCatalog;
use lifthrasir_data::{SkillData, SkillMeta, StatusIconData, StatusIconEntry};

fn test_app() -> App {
    let mut skills = SkillData::default();
    for (id, name, display_name) in [
        (322, "BA_APPLEIDUN", "The Apple of Idun"),
        (321, "BA_POEMBRAGI", "Poem of Bragi"),
    ] {
        skills.skills.insert(
            id,
            SkillMeta {
                name: name.into(),
                display_name: display_name.into(),
                ..default()
            },
        );
    }
    let mut statuses = StatusIconData::default();
    statuses.icons.insert(
        73,
        StatusIconEntry {
            image: "APPLEIDUN.tga".into(),
            name: "Apple of Idun".into(),
        },
    );
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<LocalStatuses>()
        .insert_resource(StatusIconCatalog::new(statuses))
        .insert_resource(SkillCatalog::from_skill_data(skills))
        .add_systems(Update, sync_status_icons);
    app.world_mut().spawn(StatusIconBar);
    app
}

fn perform(app: &mut App, skill_id: u32) {
    app.world_mut()
        .resource_mut::<LocalStatuses>()
        .active
        .insert(
            69,
            ActiveStatus {
                total_ms: 60_000,
                expires_at: Some(Duration::from_secs(60)),
                permanent: false,
                source_skill_id: Some(skill_id),
            },
        );
}

#[test]
fn performing_uses_the_active_skill_icon_without_an_efst_catalog_entry() {
    let mut app = test_app();
    perform(&mut app, 322);
    app.update();

    let mut icons = app.world_mut().query::<(&StatusIcon, &ImageNode)>();
    let (icon, image) = icons
        .single(app.world())
        .expect("one illustrated performing status");
    assert_eq!(
        icon.efst, 69,
        "do not invent a recipient buff on the caster"
    );
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(image.image.id())
        .unwrap();
    assert_eq!(
        path.to_string(),
        "ro://data/texture/유저인터페이스/item/ba_appleidun.bmp"
    );
    assert_eq!(icon.presentation.name, "Performing: The Apple of Idun");
}

#[test]
fn performing_refreshes_when_the_song_or_catalog_changes_but_not_on_timer_refresh() {
    let mut app = test_app();
    perform(&mut app, 322);
    app.update();
    let mut icons = app.world_mut().query_filtered::<Entity, With<StatusIcon>>();
    let original = icons.single(app.world()).unwrap();
    perform(&mut app, 322);
    app.update();
    assert_eq!(
        icons.single(app.world()).unwrap(),
        original,
        "unchanged icons stay stable"
    );

    perform(&mut app, 321);
    app.update();
    let mut icons = app.world_mut().query::<(&StatusIcon, &ImageNode)>();
    let (icon, image) = icons.single(app.world()).unwrap();
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(image.image.id())
        .unwrap();
    assert_eq!(
        path.to_string(),
        "ro://data/texture/유저인터페이스/item/ba_poembragi.bmp"
    );
    assert_eq!(icon.presentation.name, "Performing: Poem of Bragi");

    let skills = app.world_mut().remove_resource::<SkillCatalog>().unwrap();
    app.update();
    let mut icons = app.world_mut().query::<(&StatusIcon, Option<&ImageNode>)>();
    let (icon, image) = icons.single(app.world()).unwrap();
    assert!(image.is_none(), "missing artwork keeps a placeholder");
    assert_eq!(
        icon.presentation.name, "Performing",
        "the placeholder still has a useful name"
    );
    app.world_mut().insert_resource(skills);
    app.update();
    let (icon, image) = icons.single(app.world()).unwrap();
    assert!(
        image.is_some(),
        "a late skill catalog replaces the placeholder"
    );
    assert_eq!(icon.presentation.name, "Performing: Poem of Bragi");

    app.world_mut()
        .resource_mut::<LocalStatuses>()
        .active
        .remove(&69);
    app.update();
    assert_eq!(
        icons.iter(app.world()).count(),
        0,
        "ending the performance removes the icon"
    );
}
