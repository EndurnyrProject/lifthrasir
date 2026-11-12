use bevy::prelude::*;
use net_contract::commands::{HomunculusCommand as Command, HomunculusSkillTarget};
use net_contract::events::{
    ChatHeard, HomunculusResultReceived, HomunculusState, HomunculusStateReceived,
};

#[derive(Component, Default, Clone)]
pub(crate) struct HomunculusStatus;

pub(crate) fn spawn_status(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(bsn! {
            HomunculusStatus
            Text("")
            Node {
                position_type: PositionType::Absolute,
                top: px(145),
                left: px(16),
                padding: UiRect::all(px(8)),
            }
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65))
            Visibility::Hidden
            Pickable::IGNORE
        })
        .insert(ChildOf(parent));
}

fn status_text(state: &HomunculusState) -> String {
    format!(
        "{}  Lv {}\nHP {}/{}  SP {}/{}\nHunger {}  Intimacy {:.2}",
        state.name,
        state.level,
        state.hp,
        state.max_hp,
        state.sp,
        state.max_sp,
        state.hunger,
        state.intimacy_hundredths as f64 / 100.0
    )
}

pub(crate) fn update_status(
    mut states: MessageReader<HomunculusStateReceived>,
    mut results: MessageReader<HomunculusResultReceived>,
    mut status: Query<(&mut Text, &mut Visibility), With<HomunculusStatus>>,
) {
    let Ok((mut text, mut visibility)) = status.single_mut() else {
        return;
    };
    for state in states
        .read()
        .map(|event| &event.0)
        .chain(results.read().filter_map(|event| event.state.as_ref()))
    {
        text.0 = status_text(state);
        *visibility = Visibility::Inherited;
    }
}

pub(crate) fn parse_command(input: &str) -> Option<Command> {
    let words: Vec<_> = input.split_whitespace().collect();
    match words.as_slice() {
        ["/homun", "inspect"] => Some(Command::Inspect),
        ["/homun", "feed"] => Some(Command::Feed),
        ["/homun", "follow"] => Some(Command::Follow),
        ["/homun", "standby"] => Some(Command::Standby),
        ["/homun", "rest"] => Some(Command::Rest),
        ["/homun", "rename", name] => Some(Command::Rename {
            name: (*name).into(),
        }),
        ["/homun", "move", x, y] => Some(Command::Move {
            x: x.parse().ok()?,
            y: y.parse().ok()?,
        }),
        ["/homun", "attack", id] => Some(Command::Attack {
            target_id: id.parse().ok()?,
        }),
        ["/homun", "learn", id] => Some(Command::LearnSkill {
            skill_id: id.parse().ok()?,
        }),
        ["/homun", "cast", id, level, target] => Some(Command::CastSkill {
            skill_id: id.parse().ok()?,
            level: level.parse().ok()?,
            target: if *target == "self" {
                HomunculusSkillTarget::SelfTarget
            } else {
                HomunculusSkillTarget::Actor(target.parse().ok()?)
            },
        }),
        _ => None,
    }
}

pub(crate) fn show_result(
    mut results: MessageReader<HomunculusResultReceived>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for result in results.read() {
        let message = if !result.success {
            format!("Homunculus request failed (error {}).", result.error)
        } else if let Some(state) = &result.state {
            format!(
                "{} Lv {} | HP {}/{} | SP {}/{} | EXP {}/{} | Hunger {} | Intimacy {:.2} | Skill points {}",
                state.name,
                state.level,
                state.hp,
                state.max_hp,
                state.sp,
                state.max_sp,
                state.exp,
                state.next_exp,
                state.hunger,
                state.intimacy_hundredths as f64 / 100.0,
                state.skill_points
            )
        } else {
            "Homunculus request succeeded.".to_string()
        };
        chat.write(ChatHeard { gid: 0, message });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands_without_accepting_invalid_arguments() {
        assert_eq!(
            parse_command("/homun move -2 7"),
            Some(Command::Move { x: -2, y: 7 })
        );
        assert_eq!(
            parse_command("/homun cast 8001 2 self"),
            Some(Command::CastSkill {
                skill_id: 8001,
                level: 2,
                target: HomunculusSkillTarget::SelfTarget
            })
        );
        assert_eq!(parse_command("/homun attack nope"), None);
        assert_eq!(parse_command("/homun feed extra"), None);
    }
}
