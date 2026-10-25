use lifthrasir_assets::catalogs::skill::SkillCatalog;
use lifthrasir_assets::catalogs::status::catalog::StatusIconCatalog;

/// Resolved artwork and label, also used to detect a changed or newly loaded icon.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct StatusPresentation {
    pub image_path: Option<String>,
    pub name: String,
}

pub(super) fn resolve_status_presentation(
    efst: u32,
    source_skill_id: Option<u32>,
    statuses: &StatusIconCatalog,
    skills: Option<&SkillCatalog>,
) -> StatusPresentation {
    if efst == 69 {
        let skill = source_skill_id.and_then(|id| skills?.get(id));
        let image_path = source_skill_id
            .and_then(|id| skills?.icon_path(id))
            .or_else(|| statuses.icon_path(efst));
        let name = skill
            .filter(|skill| !skill.display_name.is_empty())
            .map(|skill| format!("Performing: {}", skill.display_name))
            .unwrap_or_else(|| "Performing".into());
        return StatusPresentation { image_path, name };
    }

    StatusPresentation {
        image_path: statuses.icon_path(efst),
        name: statuses
            .name(efst)
            .map(str::to_string)
            .unwrap_or_else(|| format!("#{efst}")),
    }
}
