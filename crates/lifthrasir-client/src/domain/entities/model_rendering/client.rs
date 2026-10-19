use crate::domain::entities::model_rendering::*;
use crate::domain::guild::emblems::EmblemKey;

pub(super) fn sync_actor_emblems(
    mut owners: Query<(&GuildFlag, &mut ActorEmblem, Option<&ActorVisualStatus>)>,
    mut cache: ResMut<GuildEmblemImages>,
) {
    for (flag, mut emblem, status) in &mut owners {
        let texture = if matches!(status, Some(ActorVisualStatus::Ready)) {
            EmblemKey::new(flag.guild_id, flag.emblem_id).and_then(|key| {
                cache.request(key);
                cache.cached(key)
            })
        } else {
            None
        };
        emblem.set_if_neq(ActorEmblem(texture));
    }
}

pub(super) fn enforce_model_load_failures(actors: Query<&ActorVisualStatus>) {
    for status in &actors {
        if let ActorVisualStatus::Failed(error) = status {
            panic!("{error}");
        }
    }
}
