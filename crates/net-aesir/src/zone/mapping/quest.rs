use bevy::log::warn;

use crate::envelope::Body;
use crate::proto::aesir::net;
use net_contract::dto::{HuntFilter, QuestEntry, QuestGiver, QuestObjective, QuestState};
use net_contract::events::QuestEvent;

/// The quest event a body stands for, or `None` for a body that says nothing about
/// quests or carries a state this client does not know.
pub fn quest_event(body: Body) -> Option<QuestEvent> {
    Some(match body {
        Body::QuestList(l) => QuestEvent::List(
            l.quests
                .into_iter()
                .map(entry)
                .collect::<Option<Vec<_>>>()?,
        ),
        Body::QuestAdded(a) => QuestEvent::Added(entry(a.quest?)?),
        Body::QuestRemoved(r) => QuestEvent::Removed {
            quest_id: r.quest_id,
        },
        Body::QuestStateChanged(s) => QuestEvent::StateChanged {
            quest_id: s.quest_id,
            state: state(s.state)?,
        },
        Body::QuestHuntProgress(p) => QuestEvent::HuntProgress {
            quest_id: p.quest_id,
            objective_index: p.objective_index as usize,
            count: p.count,
            needed: p.needed,
        },
        _ => return None,
    })
}

fn entry(e: net::QuestEntry) -> Option<QuestEntry> {
    Some(QuestEntry {
        quest_id: e.quest_id,
        state: state(e.state)?,
        title: e.title,
        objectives: e.objectives.into_iter().map(objective).collect(),
        deadline: u64::try_from(e.deadline).ok().filter(|&d| d != 0),
        giver: e.giver.map(giver),
    })
}

fn objective(o: net::QuestObjective) -> QuestObjective {
    QuestObjective {
        mob_id: o.mob_id,
        mob_name: o.mob_name,
        needed: o.needed,
        current: o.current,
        filter: HuntFilter {
            min_level: o.min_level,
            max_level: o.max_level,
            race: o.race,
            size: o.size,
            element: o.element,
            map: o.map,
        },
    }
}

fn giver(g: net::QuestGiver) -> QuestGiver {
    QuestGiver {
        name: g.name,
        map: g.map,
        x: g.x as u16,
        y: g.y as u16,
    }
}

fn state(code: u32) -> Option<QuestState> {
    match code {
        0 => Some(QuestState::Inactive),
        1 => Some(QuestState::Active),
        2 => Some(QuestState::Complete),
        other => {
            warn!("unknown quest state {other}; skipping");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::aesir::net;
    use net_contract::dto::{HuntFilter, QuestEntry, QuestGiver, QuestObjective, QuestState};

    fn wire_entry(state: u32) -> net::QuestEntry {
        net::QuestEntry {
            quest_id: 7,
            state,
            title: "Poring Hunt".into(),
            objectives: vec![net::QuestObjective {
                mob_id: 1002,
                mob_name: "Poring".into(),
                needed: 10,
                current: 3,
                min_level: 2,
                max_level: 9,
                race: "plant".into(),
                size: "small".into(),
                element: "water".into(),
                map: "prt_fild08".into(),
            }],
            deadline: 1_700_000_000,
            giver: Some(net::QuestGiver {
                name: "Hunter Guide".into(),
                map: "prontera".into(),
                x: 150,
                y: 180,
            }),
        }
    }

    fn entry(state: QuestState) -> QuestEntry {
        QuestEntry {
            quest_id: 7,
            state,
            title: "Poring Hunt".into(),
            objectives: vec![QuestObjective {
                mob_id: 1002,
                mob_name: "Poring".into(),
                needed: 10,
                current: 3,
                filter: HuntFilter {
                    min_level: 2,
                    max_level: 9,
                    race: "plant".into(),
                    size: "small".into(),
                    element: "water".into(),
                    map: "prt_fild08".into(),
                },
            }],
            deadline: Some(1_700_000_000),
            giver: Some(QuestGiver {
                name: "Hunter Guide".into(),
                map: "prontera".into(),
                x: 150,
                y: 180,
            }),
        }
    }

    #[test]
    fn a_quest_list_and_an_added_quest_carry_every_field() {
        let list = Body::QuestList(net::QuestList {
            quests: vec![wire_entry(1)],
        });
        assert_eq!(
            quest_event(list),
            Some(QuestEvent::List(vec![entry(QuestState::Active)]))
        );

        let added = Body::QuestAdded(net::QuestAdded {
            quest: Some(wire_entry(2)),
        });
        assert_eq!(
            quest_event(added),
            Some(QuestEvent::Added(entry(QuestState::Complete)))
        );
    }

    #[test]
    fn no_deadline_and_no_giver_become_none() {
        let wire = net::QuestEntry {
            deadline: 0,
            giver: None,
            ..wire_entry(0)
        };
        let Some(QuestEvent::Added(added)) =
            quest_event(Body::QuestAdded(net::QuestAdded { quest: Some(wire) }))
        else {
            panic!("expected an added quest");
        };
        assert_eq!((added.deadline, added.giver), (None, None));
        assert_eq!(added.state, QuestState::Inactive);
    }

    #[test]
    fn removal_state_change_and_progress_map_one_to_one() {
        assert_eq!(
            quest_event(Body::QuestRemoved(net::QuestRemoved { quest_id: 7 })),
            Some(QuestEvent::Removed { quest_id: 7 })
        );
        assert_eq!(
            quest_event(Body::QuestStateChanged(net::QuestStateChanged {
                quest_id: 7,
                state: 1,
            })),
            Some(QuestEvent::StateChanged {
                quest_id: 7,
                state: QuestState::Active,
            })
        );
        assert_eq!(
            quest_event(Body::QuestHuntProgress(net::QuestHuntProgress {
                quest_id: 7,
                objective_index: 2,
                count: 4,
                needed: 10,
            })),
            Some(QuestEvent::HuntProgress {
                quest_id: 7,
                objective_index: 2,
                count: 4,
                needed: 10,
            })
        );
    }

    #[test]
    fn an_unknown_state_is_dropped_instead_of_defaulted() {
        assert_eq!(
            quest_event(Body::QuestStateChanged(net::QuestStateChanged {
                quest_id: 7,
                state: 7,
            })),
            None
        );
        assert_eq!(
            quest_event(Body::QuestAdded(net::QuestAdded {
                quest: Some(wire_entry(7)),
            })),
            None
        );
        assert_eq!(
            quest_event(Body::QuestList(net::QuestList {
                quests: vec![wire_entry(1), wire_entry(7)],
            })),
            None
        );
    }

    #[test]
    fn other_bodies_say_nothing_about_quests() {
        assert_eq!(quest_event(Body::ChatLine(net::ChatLine::default())), None);
    }
}
