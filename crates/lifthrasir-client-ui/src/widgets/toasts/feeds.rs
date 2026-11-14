//! The game events that raise a toast: a rare item reaching the bag, a hero joining
//! the party, a whisper, the purse growing and the load crossing a weight tier.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::party::PartyState;
use net_contract::dto::{ChatChannel, PartyMemberInfo};
use net_contract::events::{ChatHeard, ItemAdded};

use super::{ShowToast, ToastAction, ToastKind};
use crate::screens::shared::{Load, thousands};

/// The item type of a card.
const CARD_TYPE: u32 = 6;
/// The refine from which gear draws the rare ring.
const RARE_REFINE: u32 = 7;
/// A whisper's text is cut to this many characters on its toast.
const WHISPER_CHARS: usize = 60;

/// A card, or gear refined far enough to draw the rare ring.
fn is_rare(item: &ItemAdded) -> bool {
    item.type_ == CARD_TYPE || (item.identified && item.refine >= RARE_REFINE)
}

pub(super) fn rare_items(
    mut added: MessageReader<ItemAdded>,
    item_db: Option<Res<ItemDb>>,
    mut toasts: MessageWriter<ShowToast>,
) {
    for item in added.read().filter(|item| is_rare(item)) {
        let Some(name) = item_db
            .as_deref()
            .and_then(|db| db.name(item.nameid, item.identified))
        else {
            continue;
        };
        let title = match item.refine {
            0 => name.to_string(),
            refine => format!("+{refine} {name}"),
        };
        toasts.write(ShowToast {
            kind: ToastKind::RareItem,
            title,
            sub: "Added to your bag".into(),
            action: None,
        });
    }
}

/// The members of `now` that the same party did not have `before`.
fn joined<'a>(
    before: &(u32, HashSet<u32>),
    party_id: u32,
    now: &'a [PartyMemberInfo],
) -> Vec<&'a PartyMemberInfo> {
    if party_id == 0 || before.0 != party_id {
        return Vec::new();
    }
    now.iter()
        .filter(|member| !before.1.contains(&member.char_id))
        .collect()
}

/// Toasts a hero who joins the party the player is already in. Joining a party
/// oneself changes the party, so its roster is remembered, not announced.
pub(super) fn party_joins(
    party: Option<Res<PartyState>>,
    jobs: Option<Res<JobSpriteRegistry>>,
    mut toasts: MessageWriter<ShowToast>,
    mut seen: Local<(u32, HashSet<u32>)>,
) {
    let Some(party) = party.filter(|party| party.is_changed()) else {
        return;
    };
    for member in joined(&seen, party.party_id, &party.members) {
        let job = jobs
            .as_deref()
            .and_then(|jobs| jobs.get_display_name(member.job_id))
            .unwrap_or("Unknown");
        toasts.write(ShowToast {
            kind: ToastKind::Party,
            title: format!("{} joined {}", member.name, party.name),
            sub: format!("Lv {} {job}", member.base_level),
            action: None,
        });
    }
    let ids = party.members.iter().map(|member| member.char_id).collect();
    *seen = (party.party_id, ids);
}

/// `text` cut to [`WHISPER_CHARS`] characters, with an ellipsis when cut.
fn excerpt(text: &str) -> String {
    if text.chars().count() <= WHISPER_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(WHISPER_CHARS).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// Toasts a whisper from someone else, with a button to answer it.
pub(super) fn whispers(
    mut heard: MessageReader<ChatHeard>,
    player: Query<&EntityName, With<LocalPlayer>>,
    mut toasts: MessageWriter<ShowToast>,
) {
    let me = player.single().map_or("", |name| name.name.as_str());
    for line in heard.read() {
        if line.channel != ChatChannel::Whisper || line.sender_name == me {
            continue;
        }
        toasts.write(ShowToast {
            kind: ToastKind::Whisper,
            title: line.sender_name.clone(),
            sub: excerpt(&line.text),
            action: Some(ToastAction::Reply(line.sender_name.clone())),
        });
    }
}

/// What of the hero's status the purse and load toasts compare: the weight limit,
/// the weight tier and the zeny.
type Carried = (u32, Load, u32);

/// The toasts a change from `before` to `now` calls for. A status that had no weight
/// limit yet was still loading, so nothing is said about it.
fn carried_toasts(before: Carried, now: Carried) -> Vec<ShowToast> {
    let ((limit, load_before, zeny_before), (_, load, zeny)) = (before, now);
    if limit == 0 {
        return Vec::new();
    }
    let mut toasts = Vec::new();
    if zeny > zeny_before {
        toasts.push(ShowToast {
            kind: ToastKind::Zeny,
            title: format!("+{} z", thousands(u64::from(zeny - zeny_before))),
            sub: String::new(),
            action: None,
        });
    }
    let weight = match (load_before, load) {
        (Load::Light, Load::Heavy) => Some((
            "You\u{2019}re carrying over 50%",
            "HP and SP stopped regenerating",
        )),
        (Load::Light | Load::Heavy, Load::Overweight) => Some((
            "You\u{2019}re carrying over 90%",
            "You can\u{2019}t attack or use skills",
        )),
        _ => None,
    };
    if let Some((title, sub)) = weight {
        toasts.push(ShowToast {
            kind: ToastKind::Weight,
            title: title.into(),
            sub: sub.into(),
            action: Some(ToastAction::OpenInventory),
        });
    }
    toasts
}

type StatusChanged = (With<LocalPlayer>, Changed<CharacterStatus>);

/// Toasts zeny gained and a weight tier crossed upwards. What a hero logs in with is
/// remembered, not announced.
pub(super) fn purse_and_load(
    player: Query<(Entity, &CharacterStatus), StatusChanged>,
    mut toasts: MessageWriter<ShowToast>,
    mut seen: Local<Option<(Entity, Carried)>>,
) {
    let Ok((hero, status)) = player.single() else {
        return;
    };
    let now = (
        status.max_weight,
        Load::of(status.weight, status.max_weight),
        status.zeny,
    );
    let Some((was, before)) = seen.replace((hero, now)) else {
        return;
    };
    if was != hero {
        return;
    }
    for toast in carried_toasts(before, now) {
        toasts.write(toast);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(type_: u32, refine: u32, identified: bool) -> ItemAdded {
        ItemAdded {
            index: 2,
            amount: 1,
            nameid: 4133,
            identified,
            attribute: 0,
            refine,
            cards: vec![],
            options: vec![],
            location: 0,
            type_,
            result: 0,
            expire_time: 0,
            look: 0,
        }
    }

    #[test]
    fn cards_and_highly_refined_gear_are_rare() {
        assert!(is_rare(&item(CARD_TYPE, 0, true)));
        assert!(is_rare(&item(5, 7, true)));
        assert!(!is_rare(&item(5, 6, true)));
        assert!(
            !is_rare(&item(5, 9, false)),
            "unidentified gear hides its refine"
        );
        assert!(!is_rare(&item(0, 0, true)));
    }

    fn member(char_id: u32) -> PartyMemberInfo {
        PartyMemberInfo {
            char_id,
            name: format!("hero {char_id}"),
            base_level: 99,
            online: true,
            map: "prontera".into(),
            job_id: 7,
            hp: 1,
            max_hp: 1,
            sp: 1,
            max_sp: 1,
            ap: 0,
            max_ap: 0,
        }
    }

    #[test]
    fn only_newcomers_to_the_same_party_have_joined() {
        let before = (7, HashSet::from_iter([1, 2]));
        let now = [member(1), member(2), member(3)];

        let newcomers: Vec<_> = joined(&before, 7, &now).iter().map(|m| m.char_id).collect();
        assert_eq!(newcomers, [3]);
        assert!(
            joined(&before, 8, &now).is_empty(),
            "the player changed party"
        );
        assert!(
            joined(&(0, HashSet::new()), 7, &now).is_empty(),
            "the player just joined"
        );
        assert!(joined(&before, 0, &[]).is_empty(), "the party is gone");
    }

    #[test]
    fn a_long_whisper_is_cut_with_an_ellipsis() {
        assert_eq!(excerpt("hi"), "hi");
        let long = "x".repeat(WHISPER_CHARS + 5);
        let cut = excerpt(&long);
        assert_eq!(cut.chars().count(), WHISPER_CHARS + 1);
        assert!(cut.ends_with('\u{2026}'));
    }

    fn kinds(before: Carried, now: Carried) -> Vec<ToastKind> {
        carried_toasts(before, now)
            .into_iter()
            .map(|toast| toast.kind)
            .collect()
    }

    #[test]
    fn zeny_gained_is_toasted_and_zeny_spent_is_not() {
        let gained = carried_toasts((2000, Load::Light, 100), (2000, Load::Light, 48_300));
        assert_eq!(gained.len(), 1);
        assert_eq!(gained[0].title, "+48,200 z");
        assert!(kinds((2000, Load::Light, 500), (2000, Load::Light, 100)).is_empty());
    }

    #[test]
    fn a_weight_tier_is_toasted_only_on_the_way_up() {
        let up = carried_toasts((2000, Load::Light, 0), (2000, Load::Heavy, 0));
        assert_eq!(up.len(), 1);
        assert_eq!(up[0].action, Some(ToastAction::OpenInventory));
        assert_eq!(
            kinds((2000, Load::Heavy, 0), (2000, Load::Overweight, 0)),
            [ToastKind::Weight]
        );
        assert!(kinds((2000, Load::Overweight, 0), (2000, Load::Heavy, 0)).is_empty());
        assert!(kinds((2000, Load::Heavy, 0), (2000, Load::Heavy, 0)).is_empty());
    }

    #[test]
    fn a_status_still_loading_raises_nothing() {
        assert!(kinds((0, Load::Light, 0), (2000, Load::Heavy, 90_000)).is_empty());
    }
}
