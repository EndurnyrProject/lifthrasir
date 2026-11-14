//! Where a job's skills sit in its tab: a top-down grid with one band of rows per
//! prerequisite depth, the requirement links drawn between them, and the prerequisite
//! chain a focused skill lights.
//!
//! NOTE: the placement is computed, not authored: the client has no tree positions.
//! Read them from the catalog here if the converter ever exports the retail ones.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use lifthrasir_client::domain::skill::SkillTreeState;

/// Grid columns of a job tab.
pub const COLUMNS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub skill_id: u32,
    pub column: u32,
    pub row: u32,
}

/// A requirement between two skills of the same job, drawn as a line from `source`
/// down to `target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub source: u32,
    pub target: u32,
    pub minimum_level: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobTree {
    pub nodes: Vec<Placement>,
    pub links: Vec<Link>,
    pub rows: u32,
}

/// The jobs that own a skill, in tab order. Job ids grow along a class line (Novice,
/// first job, second job, ...), so the id order is the progression order.
pub fn jobs(tree: &SkillTreeState) -> Vec<u32> {
    let jobs: BTreeSet<u32> = tree.skills.values().map(|node| node.job_id).collect();
    jobs.into_iter().collect()
}

/// Places the skills of `job_id`. A skill sits below everything it requires within the
/// job, in its first requirement's column when that cell is free. A depth with more
/// skills than [`COLUMNS`] wraps onto further rows.
pub fn job_tree(tree: &SkillTreeState, job_id: u32) -> JobTree {
    let mut requires: BTreeMap<u32, Vec<(u32, u32)>> = tree
        .skills
        .iter()
        .filter(|(_, node)| node.job_id == job_id)
        .map(|(&skill_id, node)| (skill_id, node.requires.clone()))
        .collect();
    let ids: BTreeSet<u32> = requires.keys().copied().collect();
    for sources in requires.values_mut() {
        sources.retain(|(source, _)| ids.contains(source));
        sources.sort_unstable();
    }

    let mut depths = HashMap::new();
    let mut bands: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &skill_id in &ids {
        let depth = depth(skill_id, &requires, &mut depths, &mut Vec::new());
        bands.entry(depth).or_default().push(skill_id);
    }

    let mut cells: HashMap<u32, (u32, u32)> = HashMap::new();
    let mut rows = 0;
    for band in bands.values() {
        let mut wanted: Vec<(u32, Option<u32>)> = band
            .iter()
            .map(|skill_id| {
                let column = requires[skill_id]
                    .first()
                    .and_then(|(source, _)| cells.get(source))
                    .map(|&(column, _)| column);
                (*skill_id, column)
            })
            .collect();
        // Skills that inherit a column go first, so a root does not take it.
        wanted.sort_by_key(|(_, column)| column.is_none());

        let mut taken: Vec<[bool; COLUMNS]> = Vec::new();
        for (skill_id, column) in wanted {
            let (row, column) = place(&mut taken, column);
            cells.insert(skill_id, (column, rows + row));
        }
        rows += taken.len() as u32;
    }

    let nodes = ids
        .iter()
        .map(|&skill_id| {
            let (column, row) = cells[&skill_id];
            Placement {
                skill_id,
                column,
                row,
            }
        })
        .collect();
    // A requirement cycle has an edge that points back up; it gets no line.
    let links = requires
        .iter()
        .flat_map(|(&target, sources)| {
            sources.iter().map(move |&(source, minimum_level)| Link {
                source,
                target,
                minimum_level,
            })
        })
        .filter(|link| cells[&link.source].1 < cells[&link.target].1)
        .collect();
    JobTree { nodes, links, rows }
}

/// The first free cell of a band, as `(row, column)`: `wanted` in the first row where
/// it is free, otherwise the leftmost free cell.
fn place(taken: &mut Vec<[bool; COLUMNS]>, wanted: Option<u32>) -> (u32, u32) {
    let mut row = 0;
    loop {
        if taken.len() == row {
            taken.push([false; COLUMNS]);
        }
        let cells = &mut taken[row];
        let free = wanted
            .filter(|&column| !cells[column as usize])
            .or_else(|| cells.iter().position(|taken| !taken).map(|c| c as u32));
        if let Some(column) = free {
            cells[column as usize] = true;
            return (row as u32, column);
        }
        row += 1;
    }
}

/// Longest same-job requirement chain above `skill_id`. An edge that closes a cycle
/// adds no depth.
fn depth(
    skill_id: u32,
    requires: &BTreeMap<u32, Vec<(u32, u32)>>,
    depths: &mut HashMap<u32, u32>,
    stack: &mut Vec<u32>,
) -> u32 {
    if let Some(&depth) = depths.get(&skill_id) {
        return depth;
    }
    if stack.contains(&skill_id) {
        return 0;
    }
    stack.push(skill_id);
    let depth = requires[&skill_id]
        .iter()
        .map(|&(source, _)| depth(source, requires, depths, stack) + 1)
        .max()
        .unwrap_or(0);
    stack.pop();
    depths.insert(skill_id, depth);
    depth
}

/// `skill_id` and everything it requires, directly or not, across jobs.
pub fn chain(tree: &SkillTreeState, skill_id: u32) -> HashSet<u32> {
    let mut chain = HashSet::new();
    let mut pending = vec![skill_id];
    while let Some(id) = pending.pop() {
        if !chain.insert(id) {
            continue;
        }
        if let Some(node) = tree.skills.get(&id) {
            pending.extend(node.requires.iter().map(|&(source, _)| source));
        }
    }
    chain
}

#[cfg(test)]
mod tests {
    use lifthrasir_client::domain::skill::{SkillNode, SkillTreeState};

    use super::*;

    /// A skill id, its job and its `(skill, level)` requirements.
    type Entry<'a> = (u32, u32, &'a [(u32, u32)]);

    fn tree(entries: &[Entry]) -> SkillTreeState {
        SkillTreeState {
            skills: entries
                .iter()
                .map(|&(skill_id, job_id, requires)| {
                    let node = SkillNode {
                        level: 0,
                        max_level: 5,
                        upgradable: true,
                        requires: requires.to_vec(),
                        req_base_level: 0,
                        req_job_level: 0,
                        sp: 0,
                        range: 0,
                        inf_type: 0,
                        job_id,
                        splash_radius: 0,
                        cast_time: vec![],
                        cooldown: vec![],
                    };
                    (skill_id, node)
                })
                .collect(),
        }
    }

    fn cell(layout: &JobTree, skill_id: u32) -> (u32, u32) {
        let node = layout
            .nodes
            .iter()
            .find(|node| node.skill_id == skill_id)
            .expect("placed skill");
        (node.column, node.row)
    }

    #[test]
    fn jobs_are_listed_once_in_id_order() {
        let tree = tree(&[(30, 7, &[]), (10, 0, &[]), (20, 1, &[]), (21, 1, &[])]);
        assert_eq!(jobs(&tree), [0, 1, 7]);
    }

    #[test]
    fn a_skill_sits_below_its_requirement_in_the_same_column() {
        let tree = tree(&[
            (10, 1, &[]),
            (11, 1, &[]),
            (20, 1, &[(11, 1)]),
            (30, 1, &[(20, 3)]),
        ]);
        let layout = job_tree(&tree, 1);

        assert_eq!(cell(&layout, 10), (0, 0));
        assert_eq!(cell(&layout, 11), (1, 0));
        assert_eq!(cell(&layout, 20), (1, 1));
        assert_eq!(cell(&layout, 30), (1, 2));
        assert_eq!(layout.rows, 3);
    }

    #[test]
    fn a_wide_depth_wraps_and_pushes_deeper_skills_down() {
        let mut entries: Vec<Entry> = (1..=8).map(|id| (id, 1, &[][..])).collect();
        entries.push((9, 1, &[(1, 1)]));
        let layout = job_tree(&tree(&entries), 1);

        assert_eq!(cell(&layout, 6), (5, 0));
        assert_eq!(cell(&layout, 7), (0, 1));
        assert_eq!(cell(&layout, 9), (0, 2));
        assert_eq!(layout.rows, 3);
    }

    #[test]
    fn only_same_job_requirements_become_links() {
        let tree = tree(&[
            (10, 1, &[]),
            (20, 1, &[(10, 3), (99, 1)]),
            (30, 2, &[(20, 5)]),
        ]);

        assert_eq!(
            job_tree(&tree, 1).links,
            [Link {
                source: 10,
                target: 20,
                minimum_level: 3
            }]
        );
        assert!(job_tree(&tree, 2).links.is_empty());
        assert_eq!(job_tree(&tree, 2).nodes.len(), 1);
    }

    #[test]
    fn a_requirement_cycle_places_every_skill_once() {
        let tree = tree(&[(10, 1, &[(20, 1)]), (20, 1, &[(10, 1)])]);
        let layout = job_tree(&tree, 1);

        assert_eq!(layout.nodes.len(), 2);
        assert_ne!(cell(&layout, 10), cell(&layout, 20));
        assert_eq!(layout.links.len(), 1);
    }

    #[test]
    fn chain_follows_requirements_across_jobs() {
        let tree = tree(&[
            (10, 1, &[]),
            (20, 1, &[(10, 1)]),
            (30, 2, &[(20, 1)]),
            (40, 2, &[]),
        ]);
        assert_eq!(chain(&tree, 30), HashSet::from([10, 20, 30]));
        assert_eq!(chain(&tree, 40), HashSet::from([40]));
    }
}
