use std::collections::HashMap;

use super::state::SkillTreeState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub tab: u32,
    pub col: u32,
    pub row: u32,
}

pub fn layout(tree: &SkillTreeState) -> HashMap<u32, Placement> {
    let mut tabs: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&skill_id, node) in &tree.skills {
        tabs.entry(node.job_id).or_default().push(skill_id);
    }

    let mut placements = HashMap::new();
    for (tab, skill_ids) in tabs {
        let in_tab: std::collections::HashSet<u32> = skill_ids.iter().copied().collect();

        let mut depths = HashMap::new();
        let mut by_row: HashMap<u32, Vec<u32>> = HashMap::new();
        for &skill_id in &skill_ids {
            let row = depth(skill_id, tree, &in_tab, &mut depths, &mut Vec::new()).unwrap_or(0);
            by_row.entry(row).or_default().push(skill_id);
        }

        for (row, mut row_skills) in by_row {
            row_skills.sort_unstable();
            for (col, skill_id) in row_skills.into_iter().enumerate() {
                placements.insert(
                    skill_id,
                    Placement {
                        tab,
                        col: col as u32,
                        row,
                    },
                );
            }
        }
    }

    placements
}

/// Longest in-tab prerequisite-chain depth. Returns `None` when the skill sits
/// on a cycle (a prereq edge points back into the in-progress stack), which the
/// caller degrades to row 0.
fn depth(
    skill_id: u32,
    tree: &SkillTreeState,
    in_tab: &std::collections::HashSet<u32>,
    depths: &mut HashMap<u32, u32>,
    stack: &mut Vec<u32>,
) -> Option<u32> {
    if let Some(&d) = depths.get(&skill_id) {
        return Some(d);
    }
    if stack.contains(&skill_id) {
        return None;
    }

    stack.push(skill_id);
    let in_tab_prereqs: Vec<u32> = tree
        .skills
        .get(&skill_id)
        .map(|node| {
            node.requires
                .iter()
                .map(|&(prereq, _)| prereq)
                .filter(|prereq| in_tab.contains(prereq))
                .collect()
        })
        .unwrap_or_default();

    let mut result = Some(0);
    for prereq in in_tab_prereqs {
        match depth(prereq, tree, in_tab, depths, stack) {
            Some(d) => result = result.map(|r| r.max(d + 1)),
            None => result = None,
        }
    }
    stack.pop();

    if let Some(d) = result {
        depths.insert(skill_id, d);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::skill::state::SkillNode;

    fn node(job_id: u32, requires: Vec<(u32, u32)>) -> SkillNode {
        SkillNode {
            level: 0,
            max_level: 5,
            upgradable: true,
            requires,
            req_base_level: 0,
            req_job_level: 0,
            sp: 0,
            range: 0,
            inf_type: 0,
            job_id,
            splash_radius: 0,
        }
    }

    fn tree(entries: &[(u32, SkillNode)]) -> SkillTreeState {
        let mut state = SkillTreeState::default();
        for (id, n) in entries {
            state.skills.insert(*id, node(n.job_id, n.requires.clone()));
        }
        state
    }

    #[test]
    fn linear_chain_increases_rows() {
        let state = tree(&[
            (1, node(7, vec![])),
            (2, node(7, vec![(1, 1)])),
            (3, node(7, vec![(2, 1)])),
        ]);
        let placements = layout(&state);
        assert_eq!(placements[&1].row, 0);
        assert_eq!(placements[&2].row, 1);
        assert_eq!(placements[&3].row, 2);
    }

    #[test]
    fn same_row_packs_by_skill_id() {
        let state = tree(&[(5, node(7, vec![])), (3, node(7, vec![]))]);
        let placements = layout(&state);
        assert_eq!((placements[&3].row, placements[&3].col), (0, 0));
        assert_eq!((placements[&5].row, placements[&5].col), (0, 1));
    }

    #[test]
    fn different_job_ids_land_in_different_tabs() {
        let state = tree(&[(1, node(7, vec![])), (2, node(9, vec![]))]);
        let placements = layout(&state);
        assert_ne!(placements[&1].tab, placements[&2].tab);
        assert_eq!(placements[&1].tab, 7);
        assert_eq!(placements[&2].tab, 9);
    }

    #[test]
    fn cross_tab_prereq_does_not_push_row() {
        let state = tree(&[(1, node(7, vec![])), (2, node(9, vec![(1, 1)]))]);
        let placements = layout(&state);
        assert_eq!(placements[&2].row, 0);
    }

    #[test]
    fn cycle_degrades_to_row_zero_without_hanging() {
        let state = tree(&[(1, node(7, vec![(2, 1)])), (2, node(7, vec![(1, 1)]))]);
        let placements = layout(&state);
        assert_eq!(placements[&1].row, 0);
        assert_eq!(placements[&2].row, 0);
    }
}
