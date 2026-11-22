//! The draft of a companion's tactics. The server owns the configuration and takes it
//! whole, so every edit changes one field of a full copy and leaves the rest as it was
//! received, including the rules the window does not show.

use lifthrasir_client::domain::homunculus::{HomunculusSkillSpec, homunculus_skill_spec};
use net_contract::dto::{
    HomunculusAiConfig as Config, HomunculusAiSkillConfig as Rule, HomunculusAiSkillMode as Mode,
    HomunculusAiStance as Stance, HomunculusHpRange,
};

/// An on/off rule of the configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Flag {
    #[default]
    Join,
    Retaliate,
    AvoidBoss,
    AutoFeed,
}

impl Flag {
    pub(super) fn read(self, config: &Config) -> bool {
        match self {
            Flag::Join => config.join_owner_target,
            Flag::Retaliate => config.retaliate,
            Flag::AvoidBoss => config.avoid_bosses,
            Flag::AutoFeed => config.auto_feed,
        }
    }

    fn flip(self, config: &mut Config) {
        let flag = match self {
            Flag::Join => &mut config.join_owner_target,
            Flag::Retaliate => &mut config.retaliate,
            Flag::AvoidBoss => &mut config.avoid_bosses,
            Flag::AutoFeed => &mut config.auto_feed,
        };
        *flag = !*flag;
    }
}

/// A list of monster class ids the companion is held to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum MobList {
    /// When not empty, the only monsters it attacks.
    #[default]
    Only,
    Never,
}

impl MobList {
    fn ids(self, config: &mut Config) -> &mut Vec<u32> {
        match self {
            MobList::Only => &mut config.allowed_mob_class_ids,
            MobList::Never => &mut config.denied_mob_class_ids,
        }
    }
}

/// A number of the configuration, set with a slider. The HP dials carry their rule's
/// skill id and are off at 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Dial {
    #[default]
    Leash,
    FeedAt,
    SpReserve,
    SelfHp(u32),
    OwnerHp(u32),
    /// The upper end of the target's HP range. The lower end stays as received.
    TargetHp(u32),
}

impl Dial {
    /// The lowest and highest value the server takes, and the slider's step.
    fn span(self) -> (u32, u32, u32) {
        match self {
            Dial::Leash => (2, 14, 1),
            Dial::FeedAt => (1, 75, 1),
            Dial::SpReserve | Dial::SelfHp(_) | Dial::OwnerHp(_) | Dial::TargetHp(_) => (0, 100, 5),
        }
    }

    pub(super) fn read(self, config: &Config) -> u32 {
        let rule = |id| config.skills.iter().find(|rule| rule.skill_id == id);
        match self {
            Dial::Leash => config.leash_distance,
            Dial::FeedAt => config.auto_feed_threshold,
            Dial::SpReserve => config.auto_cast_sp_reserve_percent,
            Dial::SelfHp(id) => rule(id)
                .and_then(|rule| rule.self_hp_threshold)
                .unwrap_or(0),
            Dial::OwnerHp(id) => rule(id)
                .and_then(|rule| rule.owner_hp_threshold)
                .unwrap_or(0),
            Dial::TargetHp(id) => rule(id)
                .and_then(|rule| rule.target_hp_range)
                .map_or(0, |range| range.max_percent),
        }
    }

    fn write(self, config: &mut Config, value: u32) {
        let set = (value > 0).then_some(value);
        let (Dial::SelfHp(id) | Dial::OwnerHp(id) | Dial::TargetHp(id)) = self else {
            match self {
                Dial::Leash => config.leash_distance = value,
                Dial::FeedAt => config.auto_feed_threshold = value,
                _ => config.auto_cast_sp_reserve_percent = value,
            }
            return;
        };
        let Some(rule) = config.skills.iter_mut().find(|rule| rule.skill_id == id) else {
            return;
        };
        match self {
            Dial::SelfHp(_) => rule.self_hp_threshold = set,
            Dial::OwnerHp(_) => rule.owner_hp_threshold = set,
            _ => {
                let low = rule.target_hp_range.map_or(0, |range| range.min_percent);
                rule.target_hp_range = set.map(|max_percent| HomunculusHpRange {
                    min_percent: low.min(max_percent),
                    max_percent,
                });
            }
        }
    }

    /// How far along its slider the dial is, from 0 to 1.
    pub(super) fn fraction(self, config: &Config) -> f32 {
        let (low, high, _) = self.span();
        (self.read(config).clamp(low, high) - low) as f32 / (high - low) as f32
    }

    /// The value at `fraction` of the slider, on a step.
    fn at(self, fraction: f32) -> u32 {
        let (low, high, step) = self.span();
        let steps = (fraction.clamp(0.0, 1.0) * (high - low) as f32 / step as f32).round() as u32;
        (low + steps * step).min(high)
    }

    pub(super) fn readout(self, config: &Config) -> String {
        let value = self.read(config);
        match self {
            Dial::Leash => format!("{value} cells"),
            Dial::FeedAt => value.to_string(),
            Dial::SpReserve => format!("{value}%"),
            _ if value == 0 => "Off".into(),
            _ => format!("{value}%"),
        }
    }
}

/// Whether the window can edit this rule. Passive skills are never cast.
pub(super) fn editable(rule: &Rule) -> bool {
    matches!(
        homunculus_skill_spec(rule.skill_id),
        Some(HomunculusSkillSpec::Enemy | HomunculusSkillSpec::SelfTarget)
    )
}

/// Whether every value of the configuration is one the client knows.
pub(super) fn known(config: &Config) -> bool {
    matches!(
        config.stance,
        Stance::Passive | Stance::Assist | Stance::Aggressive
    ) && config.skills.iter().all(|rule| {
        homunculus_skill_spec(rule.skill_id).is_some()
            && matches!(rule.mode, Mode::Manual | Mode::Auto)
    })
}

/// The rules the window shows, in the order the server runs them: highest priority
/// first, then lowest skill id.
pub(super) fn rules(config: &Config) -> Vec<&Rule> {
    let mut rules: Vec<_> = config.skills.iter().filter(|rule| editable(rule)).collect();
    rules.sort_by_key(|rule| (std::cmp::Reverse(rule.priority), rule.skill_id));
    rules
}

/// The HP conditions the server accepts on an automatic rule.
pub(super) fn dials(rule: &Rule) -> Vec<Dial> {
    let id = rule.skill_id;
    let mut dials = vec![Dial::SelfHp(id), Dial::OwnerHp(id)];
    if homunculus_skill_spec(id) == Some(HomunculusSkillSpec::Enemy) {
        dials.push(Dial::TargetHp(id));
    }
    dials
}

#[derive(Default)]
pub(super) struct Editor {
    companion: u64,
    /// The server's configuration the draft started from.
    base: Option<Config>,
    draft: Option<Config>,
    /// The draft last sent, until the server answers with it.
    submitted: Option<Config>,
}

impl Editor {
    pub(super) fn reset(&mut self, id: u64, config: &Config) {
        self.companion = id;
        self.base = Some(config.clone());
        self.draft = Some(config.clone());
        self.submitted = None;
    }

    /// Whether the server's configuration is no longer the one the draft started from.
    pub(super) fn stale(&self, config: &Config) -> bool {
        self.base.as_ref() != Some(config)
    }

    pub(super) fn dirty(&self) -> bool {
        self.draft != self.base
    }

    pub(super) fn draft(&self) -> Option<&Config> {
        self.draft.as_ref()
    }

    /// Whether a draft was sent and the server has not answered with it yet.
    pub(super) fn sent(&self) -> bool {
        self.submitted.is_some()
    }

    /// Takes the server's configuration unless that would throw an edit away: for
    /// another companion, once the sent draft lands, or while nothing was changed.
    pub(super) fn follow(&mut self, id: u64, config: &Config) {
        let landed = self.submitted.as_ref() == Some(config);
        if self.companion != id || landed || !self.dirty() && self.stale(config) {
            self.reset(id, config);
        }
    }

    fn edit(&mut self, change: impl FnOnce(&mut Config)) {
        if let Some(draft) = &mut self.draft {
            change(draft);
        }
    }

    pub(super) fn set_stance(&mut self, stance: Stance) {
        self.edit(|draft| draft.stance = stance);
    }

    pub(super) fn flip(&mut self, flag: Flag) {
        self.edit(|draft| flag.flip(draft));
    }

    pub(super) fn set_dial(&mut self, dial: Dial, fraction: f32) {
        self.edit(|draft| dial.write(draft, dial.at(fraction)));
    }

    /// Sets how a skill is cast. The server takes no HP condition on a manual rule.
    pub(super) fn set_mode(&mut self, skill_id: u32, mode: Mode) {
        self.edit(|draft| {
            let Some(rule) = draft
                .skills
                .iter_mut()
                .find(|rule| rule.skill_id == skill_id)
            else {
                return;
            };
            rule.mode = mode;
            if mode == Mode::Manual {
                rule.self_hp_threshold = None;
                rule.owner_hp_threshold = None;
                rule.target_hp_range = None;
            }
        });
    }

    /// Moves a rule one place up. The shown rules are renumbered from the new order,
    /// which also settles ties between equal priorities.
    pub(super) fn raise(&mut self, skill_id: u32) {
        self.edit(|draft| {
            let mut order: Vec<u32> = rules(draft).iter().map(|rule| rule.skill_id).collect();
            let Some(place) = order.iter().position(|id| *id == skill_id) else {
                return;
            };
            if place == 0 {
                return;
            }
            order.swap(place - 1, place);
            for rule in &mut draft.skills {
                if let Some(place) = order.iter().position(|id| *id == rule.skill_id) {
                    rule.priority = (order.len() - place) as u32;
                }
            }
        });
    }

    /// Adds a monster class id to a list, which stays sorted and free of repeats.
    pub(super) fn add_mob(&mut self, list: MobList, id: u32) {
        if id == 0 {
            return;
        }
        self.edit(|draft| {
            let ids = list.ids(draft);
            if !ids.contains(&id) {
                ids.push(id);
                ids.sort_unstable();
            }
        });
    }

    pub(super) fn drop_mob(&mut self, list: MobList, id: u32) {
        self.edit(|draft| list.ids(draft).retain(|kept| *kept != id));
    }

    /// The draft to send, when it differs from what the server has.
    pub(super) fn teach(&mut self) -> Option<Config> {
        if !self.dirty() {
            return None;
        }
        self.submitted = self.draft.clone();
        self.draft.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(skill_id: u32, priority: u32) -> Rule {
        Rule {
            skill_id,
            mode: Mode::Auto,
            priority,
            ..Default::default()
        }
    }

    #[test]
    fn tactics_blocks_apply_after_skill_rows_change() {
        let original = Config {
            stance: Stance::Assist,
            ..Default::default()
        };
        let mut editor = Editor::default();
        editor.reset(1, &original);
        let mut incoming = original.clone();
        incoming.skills.push(Rule {
            skill_id: 8001,
            mode: Mode::Manual,
            ..Default::default()
        });
        assert!(!editor.stale(&original));
        assert!(editor.stale(&incoming));
    }

    #[test]
    fn stance_only_edit_preserves_hidden_ai_fields() {
        let original = Config {
            stance: Stance::Assist,
            leash_distance: 8,
            auto_feed_threshold: 25,
            allowed_mob_class_ids: vec![1002],
            denied_mob_class_ids: vec![1004],
            auto_cast_sp_reserve_percent: 12,
            skills: vec![
                Rule {
                    skill_id: 8009,
                    mode: Mode::Auto,
                    priority: 7,
                    target_hp_range: Some(HomunculusHpRange {
                        min_percent: 0,
                        max_percent: 45,
                    }),
                    ..Default::default()
                },
                Rule {
                    skill_id: 8012,
                    mode: Mode::Auto,
                    priority: 4,
                    owner_hp_threshold: Some(25),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut editor = Editor::default();
        editor.reset(1, &original);

        editor.set_stance(Stance::Passive);

        assert_eq!(
            editor.teach(),
            Some(Config {
                stance: Stance::Passive,
                ..original
            })
        );
    }

    #[test]
    fn dials_keep_threshold_presence_and_mob_lists_reject_duplicate_ids() {
        let mut editor = Editor::default();
        editor.reset(
            1,
            &Config {
                skills: vec![rule(8009, 1)],
                ..Default::default()
            },
        );

        editor.set_dial(Dial::OwnerHp(8009), 0.62);
        editor.set_dial(Dial::TargetHp(8009), 0.4);
        editor.set_dial(Dial::SelfHp(8009), 0.3);
        editor.set_dial(Dial::SelfHp(8009), 0.0);
        editor.add_mob(MobList::Never, 3);
        editor.add_mob(MobList::Never, 1);
        editor.add_mob(MobList::Never, 3);
        editor.add_mob(MobList::Never, 0);

        let draft = editor.draft().unwrap();
        assert_eq!(draft.skills[0].owner_hp_threshold, Some(60));
        assert_eq!(draft.skills[0].self_hp_threshold, None);
        assert_eq!(
            draft.skills[0].target_hp_range,
            Some(HomunculusHpRange {
                min_percent: 0,
                max_percent: 40
            })
        );
        assert_eq!(draft.denied_mob_class_ids, [1, 3]);
    }

    #[test]
    fn a_manual_rule_drops_its_hp_conditions() {
        let mut editor = Editor::default();
        editor.reset(
            1,
            &Config {
                skills: vec![Rule {
                    owner_hp_threshold: Some(60),
                    ..rule(8001, 1)
                }],
                ..Default::default()
            },
        );

        editor.set_mode(8001, Mode::Manual);

        let rule = &editor.draft().unwrap().skills[0];
        assert_eq!(rule.mode, Mode::Manual);
        assert_eq!(rule.owner_hp_threshold, None);
    }

    #[test]
    fn raising_a_rule_runs_it_before_the_one_above_even_on_a_tie() {
        let mut editor = Editor::default();
        // 8003 is passive: it is not shown and keeps its priority.
        editor.reset(
            1,
            &Config {
                skills: vec![rule(8001, 5), rule(8002, 5), rule(8003, 9), rule(8009, 2)],
                ..Default::default()
            },
        );

        editor.raise(8002);

        let draft = editor.draft().unwrap();
        let order: Vec<u32> = rules(draft).iter().map(|rule| rule.skill_id).collect();
        assert_eq!(order, [8002, 8001, 8009]);
        assert_eq!(draft.skills[2].priority, 9);
    }

    #[test]
    fn a_clean_draft_follows_the_server_and_an_edited_one_does_not() {
        let original = Config {
            leash_distance: 8,
            ..Default::default()
        };
        let pushed = Config {
            leash_distance: 10,
            ..Default::default()
        };
        let mut editor = Editor::default();
        editor.follow(1, &original);
        editor.follow(1, &pushed);
        assert_eq!(editor.draft(), Some(&pushed));

        editor.flip(Flag::Retaliate);
        editor.follow(1, &original);

        assert!(editor.stale(&original));
        assert!(editor.draft().unwrap().retaliate);
    }
}
