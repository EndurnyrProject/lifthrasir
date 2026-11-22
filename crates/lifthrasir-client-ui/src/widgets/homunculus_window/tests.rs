use super::*;

#[test]
fn care_confirmation_is_bound_to_durable_id() {
    let prompt = Confirmation {
        id: 31,
        delete: true,
    };
    let mut companion = HomunculusSnapshot {
        durable_id: 31,
        ..default()
    };
    assert_eq!(
        confirm(prompt, &companion),
        Some(Action::Delete { confirmed: true })
    );
    companion.durable_id = 32;
    assert_eq!(confirm(prompt, &companion), None);
    let feed = Confirmation {
        id: 31,
        delete: false,
    };
    assert_eq!(confirm(feed, &companion), None);
}

#[test]
fn status_and_context_menu_are_owner_scoped() {
    let state = HomunculusState::default();
    let actor = Entity::from_raw_u32(7).unwrap();
    assert!(!owns_pick(&state, actor, Some(&ActorVisualRoot(actor))));
    assert_eq!(
        status(&state, std::time::Duration::ZERO, None),
        "Loading companion…"
    );
}
