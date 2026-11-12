//! What each skill of the open job tab looks like right now. [`compute`] derives it
//! from the tree, the plan and the pointer; the systems in [`super::scene`] paint it
//! onto the spawned nodes without respawning them.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use lifthrasir_client::domain::skill::{Form, SkillTreeState, form};

use crate::widgets::skills_window::{
    SkillFilter, SkillGateSnapshot, SkillPanelStaging, SkillPanelUi, active_job, layout,
};

/// The outline of a skill slot, strongest last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
    Rest,
    Hover,
    Max,
    /// A requirement of the hovered skill.
    Chain,
    Selected,
}

/// How far a skill is pushed into the background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dim {
    None,
    /// Outside the hovered skill's requirement chain.
    Unrelated,
    /// Hidden by the Active/Passive filter.
    Faded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Look {
    /// Learned level, without the plan.
    pub level: u32,
    pub staged: u32,
    pub max: u32,
    /// Not learned and its requirements are not met.
    pub locked: bool,
    pub edge: Edge,
    pub dim: Dim,
    pub can_raise: bool,
    pub can_lower: bool,
}

impl Look {
    pub fn total(&self) -> u32 {
        self.level + self.staged
    }
}

#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub(super) struct SkillLooks {
    pub skills: HashMap<u32, Look>,
    /// The focused (hovered, else selected) skill and all it requires.
    pub chain: HashSet<u32>,
    /// Jobs with planned levels whose tab is not the open one.
    pub planned_jobs: HashSet<u32>,
}

pub(super) fn looks(
    tree: &SkillTreeState,
    ui: &SkillPanelUi,
    staging: &SkillPanelStaging,
    gates: &SkillGateSnapshot,
) -> SkillLooks {
    let job = active_job(ui, tree);
    let chain = ui
        .hovered
        .or(ui.selected)
        .map(|focus| layout::chain(tree, focus))
        .unwrap_or_default();
    let skills = tree
        .skills
        .iter()
        .filter(|(_, node)| Some(node.job_id) == job)
        .map(|(&skill_id, node)| {
            let staged = staging.staged(skill_id);
            let total = node.level + staged;
            let locked = total == 0 && !staging.requirements_met(skill_id, tree);
            let edge = if ui.selected == Some(skill_id) {
                Edge::Selected
            } else if ui.hovered.is_some_and(|hovered| hovered != skill_id)
                && chain.contains(&skill_id)
            {
                Edge::Chain
            } else if node.max_level > 0 && total >= node.max_level {
                Edge::Max
            } else if ui.hovered == Some(skill_id) && !locked {
                Edge::Hover
            } else {
                Edge::Rest
            };
            let dim = if !ui.filter.shows(form(node.inf_type)) {
                Dim::Faded
            } else if ui.hovered.is_some() && !chain.contains(&skill_id) {
                Dim::Unrelated
            } else {
                Dim::None
            };
            let look = Look {
                level: node.level,
                staged,
                max: node.max_level,
                locked,
                edge,
                dim,
                can_raise: gates.can_raise(skill_id, tree, staging),
                can_lower: staging.can_lower(skill_id, tree),
            };
            (skill_id, look)
        })
        .collect();
    let planned_jobs = tree
        .skills
        .iter()
        .filter(|(skill_id, node)| staging.staged(**skill_id) > 0 && Some(node.job_id) != job)
        .map(|(_, node)| node.job_id)
        .collect();
    SkillLooks {
        skills,
        chain,
        planned_jobs,
    }
}

impl SkillFilter {
    pub(super) fn shows(self, form: Form) -> bool {
        match self {
            Self::All => true,
            Self::Active => form != Form::Passive,
            Self::Passive => form == Form::Passive,
        }
    }
}

pub(super) fn compute(
    tree: Res<SkillTreeState>,
    ui: Res<SkillPanelUi>,
    staging: Res<SkillPanelStaging>,
    gates: Res<SkillGateSnapshot>,
    mut shown: ResMut<SkillLooks>,
) {
    *shown = looks(&tree, &ui, &staging, &gates);
}
