use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use game_engine::domain::skill::SkillTreeState;

const NODE_WIDTH: f32 = 62.0;
const NODE_HEIGHT: f32 = 82.0;
const BAND_HEADER_HEIGHT: f32 = 32.0;
const BAND_PADDING: f32 = 12.0;
const COLUMN_GAP: f32 = 28.0;
const ROW_GAP: f32 = 10.0;
const BAND_GAP: f32 = 24.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Requirement {
    pub skill_id: u32,
    pub minimum_level: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkillTopology {
    pub skill_id: u32,
    pub job_id: u32,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobBand {
    pub job_id: u32,
    pub label: Option<String>,
    pub x: f32,
    pub width: f32,
    pub height: f32,
    pub cycle_break: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodePlacement {
    pub skill_id: u32,
    pub job_id: u32,
    pub column: u32,
    pub row: u32,
    pub bounds: Bounds,
    pub cycle_fallback: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connector {
    pub source: u32,
    pub target: u32,
    pub minimum_level: u32,
    pub segments: [Segment; 3],
    pub backlink: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFocus {
    pub focused: u32,
    pub prerequisite_nodes: BTreeSet<u32>,
    pub prerequisite_edges: BTreeSet<(u32, u32)>,
    pub unlock_nodes: BTreeSet<u32>,
    pub unlock_edges: BTreeSet<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeLayout {
    pub topology: Vec<SkillTopology>,
    pub bands: Vec<JobBand>,
    pub nodes: Vec<NodePlacement>,
    pub connectors: Vec<Connector>,
    pub width: f32,
    pub height: f32,
}

impl TreeLayout {
    pub fn new(tree: &SkillTreeState, job_labels: &HashMap<u32, String>) -> Self {
        let topology = topology(tree);
        let (job_order, cycle_breaks) = order_jobs(&topology, job_labels);
        let (mut bands, mut nodes) = place_nodes(&topology, &job_order, &cycle_breaks, job_labels);
        let height = bands.iter().map(|band| band.height).fold(0.0, f32::max);
        for band in &mut bands {
            band.height = height;
        }
        let width = bands.last().map_or(0.0, |band| band.x + band.width);
        let connectors = route_connectors(&topology, &nodes);
        nodes.sort_unstable_by_key(|node| node.skill_id);

        Self {
            topology,
            bands,
            nodes,
            connectors,
            width,
            height,
        }
    }
}

pub fn focus(tree: &SkillTreeState, focused: u32) -> Option<SkillFocus> {
    focus_topology(&topology(tree), focused)
}

fn focus_topology(topology: &[SkillTopology], focused: u32) -> Option<SkillFocus> {
    let skills: BTreeMap<_, _> = topology
        .iter()
        .map(|skill| (skill.skill_id, skill))
        .collect();
    skills.get(&focused)?;

    let mut prerequisite_nodes = BTreeSet::new();
    let mut prerequisite_edges = BTreeSet::new();
    let mut visited = BTreeSet::from([focused]);
    let mut pending = vec![focused];
    while let Some(target) = pending.pop() {
        for requirement in &skills[&target].requirements {
            let source = requirement.skill_id;
            if !skills.contains_key(&source) {
                continue;
            }
            prerequisite_edges.insert((source, target));
            if source != focused {
                prerequisite_nodes.insert(source);
            }
            if visited.insert(source) {
                pending.push(source);
            }
        }
    }

    let mut unlock_nodes = BTreeSet::new();
    let mut unlock_edges = BTreeSet::new();
    for (&target, skill) in &skills {
        if target != focused
            && skill
                .requirements
                .iter()
                .any(|requirement| requirement.skill_id == focused)
        {
            unlock_nodes.insert(target);
            unlock_edges.insert((focused, target));
        }
    }

    Some(SkillFocus {
        focused,
        prerequisite_nodes,
        prerequisite_edges,
        unlock_nodes,
        unlock_edges,
    })
}

fn topology(tree: &SkillTreeState) -> Vec<SkillTopology> {
    let mut skills: Vec<_> = tree
        .skills
        .iter()
        .map(|(&skill_id, node)| {
            let mut requirements: Vec<_> = node
                .requires
                .iter()
                .map(|&(skill_id, minimum_level)| Requirement {
                    skill_id,
                    minimum_level,
                })
                .collect();
            requirements.sort_unstable_by_key(|requirement| {
                (requirement.skill_id, requirement.minimum_level)
            });
            SkillTopology {
                skill_id,
                job_id: node.job_id,
                requirements,
            }
        })
        .collect();
    skills.sort_unstable_by_key(|skill| skill.skill_id);
    skills
}

fn order_jobs(
    topology: &[SkillTopology],
    labels: &HashMap<u32, String>,
) -> (Vec<u32>, HashSet<u32>) {
    let jobs: BTreeSet<_> = topology.iter().map(|skill| skill.job_id).collect();
    let skill_jobs: HashMap<_, _> = topology
        .iter()
        .map(|skill| (skill.skill_id, skill.job_id))
        .collect();
    let edges: BTreeSet<_> = topology
        .iter()
        .flat_map(|target| {
            target.requirements.iter().filter_map(|requirement| {
                let source_job = skill_jobs.get(&requirement.skill_id).copied()?;
                (source_job != target.job_id).then_some((source_job, target.job_id))
            })
        })
        .collect();
    let mut indegree: BTreeMap<_, _> = jobs.iter().map(|&job_id| (job_id, 0_u32)).collect();
    for &(_, target) in &edges {
        indegree.entry(target).and_modify(|degree| *degree += 1);
    }

    let mut remaining = jobs;
    let mut order = Vec::with_capacity(remaining.len());
    let mut cycle_breaks = HashSet::new();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .copied()
            .filter(|job_id| indegree[job_id] == 0)
            .min_by(|left, right| job_key(*left, labels).cmp(&job_key(*right, labels)));
        let job_id = ready.unwrap_or_else(|| {
            let job_id = remaining
                .iter()
                .copied()
                .min_by(|left, right| job_key(*left, labels).cmp(&job_key(*right, labels)))
                .expect("remaining jobs are non-empty");
            cycle_breaks.insert(job_id);
            job_id
        });

        remaining.remove(&job_id);
        order.push(job_id);
        for &(_, target) in edges.iter().filter(|(source, _)| *source == job_id) {
            *indegree
                .get_mut(&target)
                .expect("edge target is a runtime job") -= 1;
        }
    }
    (order, cycle_breaks)
}

fn job_key(job_id: u32, labels: &HashMap<u32, String>) -> (Option<&str>, u32) {
    (labels.get(&job_id).map(String::as_str), job_id)
}

fn place_nodes(
    topology: &[SkillTopology],
    job_order: &[u32],
    cycle_breaks: &HashSet<u32>,
    labels: &HashMap<u32, String>,
) -> (Vec<JobBand>, Vec<NodePlacement>) {
    let by_id: HashMap<_, _> = topology
        .iter()
        .map(|skill| (skill.skill_id, skill))
        .collect();
    let mut x = 0.0;
    let mut bands = Vec::with_capacity(job_order.len());
    let mut nodes = Vec::with_capacity(topology.len());

    for &job_id in job_order {
        let skill_ids: BTreeSet<_> = topology
            .iter()
            .filter(|skill| skill.job_id == job_id)
            .map(|skill| skill.skill_id)
            .collect();
        let mut depths = HashMap::new();
        let mut cycle_fallbacks = HashSet::new();
        for &skill_id in &skill_ids {
            depth(
                skill_id,
                job_id,
                &by_id,
                &mut depths,
                &mut cycle_fallbacks,
                &mut Vec::new(),
            );
        }

        let mut columns: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for &skill_id in &skill_ids {
            columns
                .entry(depths.get(&skill_id).copied().unwrap_or(0))
                .or_default()
                .push(skill_id);
        }
        let column_count = columns.keys().next_back().copied().unwrap_or(0) + 1;
        let row_count = columns.values().map(Vec::len).max().unwrap_or(0) as u32;
        let width = BAND_PADDING * 2.0
            + column_count as f32 * NODE_WIDTH
            + column_count.saturating_sub(1) as f32 * COLUMN_GAP;
        let height = BAND_HEADER_HEIGHT
            + BAND_PADDING * 2.0
            + row_count as f32 * NODE_HEIGHT
            + row_count.saturating_sub(1) as f32 * ROW_GAP;

        for (column, skill_ids) in columns {
            for (row, skill_id) in skill_ids.into_iter().enumerate() {
                nodes.push(NodePlacement {
                    skill_id,
                    job_id,
                    column,
                    row: row as u32,
                    bounds: Bounds {
                        x: x + BAND_PADDING + column as f32 * (NODE_WIDTH + COLUMN_GAP),
                        y: BAND_HEADER_HEIGHT + BAND_PADDING + row as f32 * (NODE_HEIGHT + ROW_GAP),
                        width: NODE_WIDTH,
                        height: NODE_HEIGHT,
                    },
                    cycle_fallback: cycle_fallbacks.contains(&skill_id),
                });
            }
        }
        bands.push(JobBand {
            job_id,
            label: labels.get(&job_id).cloned(),
            x,
            width,
            height,
            cycle_break: cycle_breaks.contains(&job_id),
        });
        x += width + BAND_GAP;
    }
    (bands, nodes)
}

fn depth(
    skill_id: u32,
    job_id: u32,
    by_id: &HashMap<u32, &SkillTopology>,
    depths: &mut HashMap<u32, u32>,
    cycle_fallbacks: &mut HashSet<u32>,
    stack: &mut Vec<u32>,
) -> Option<u32> {
    if let Some(&depth) = depths.get(&skill_id) {
        return Some(depth);
    }
    if let Some(cycle_start) = stack.iter().position(|&id| id == skill_id) {
        cycle_fallbacks.extend(stack[cycle_start..].iter().copied());
        return None;
    }
    stack.push(skill_id);

    let skill = by_id.get(&skill_id)?;
    let mut result = Some(0);
    for requirement in &skill.requirements {
        if by_id
            .get(&requirement.skill_id)
            .is_none_or(|source| source.job_id != job_id)
        {
            continue;
        }
        if let Some(source_depth) = depth(
            requirement.skill_id,
            job_id,
            by_id,
            depths,
            cycle_fallbacks,
            stack,
        ) {
            result = result.map(|current| current.max(source_depth + 1));
        }
    }
    stack.pop();
    if let Some(depth) = result {
        depths.insert(skill_id, depth);
    }
    result
}

fn route_connectors(topology: &[SkillTopology], nodes: &[NodePlacement]) -> Vec<Connector> {
    let placements: HashMap<_, _> = nodes.iter().map(|node| (node.skill_id, node)).collect();
    let mut connectors = Vec::new();
    for target in topology {
        let Some(target_node) = placements.get(&target.skill_id) else {
            continue;
        };
        for requirement in &target.requirements {
            let Some(source_node) = placements.get(&requirement.skill_id) else {
                continue;
            };
            let source = Point {
                x: source_node.bounds.x + source_node.bounds.width,
                y: source_node.bounds.y + source_node.bounds.height / 2.0,
            };
            let target = Point {
                x: target_node.bounds.x,
                y: target_node.bounds.y + target_node.bounds.height / 2.0,
            };
            let middle_x = (source.x + target.x) / 2.0;
            connectors.push(Connector {
                source: requirement.skill_id,
                target: target_node.skill_id,
                minimum_level: requirement.minimum_level,
                segments: [
                    Segment {
                        start: source,
                        end: Point {
                            x: middle_x,
                            y: source.y,
                        },
                    },
                    Segment {
                        start: Point {
                            x: middle_x,
                            y: source.y,
                        },
                        end: Point {
                            x: middle_x,
                            y: target.y,
                        },
                    },
                    Segment {
                        start: Point {
                            x: middle_x,
                            y: target.y,
                        },
                        end: target,
                    },
                ],
                backlink: target.x <= source.x,
            });
        }
    }
    connectors.sort_unstable_by_key(|connector| {
        (connector.target, connector.source, connector.minimum_level)
    });
    connectors
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use game_engine::domain::skill::{SkillNode, SkillTreeState};

    use super::{TreeLayout, focus};

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
        SkillTreeState {
            skills: entries
                .iter()
                .map(|(id, skill)| (*id, node(skill.job_id, skill.requires.clone())))
                .collect(),
        }
    }

    fn band_ids(layout: &TreeLayout) -> Vec<u32> {
        layout.bands.iter().map(|band| band.job_id).collect()
    }

    fn placement(layout: &TreeLayout, skill_id: u32) -> &super::NodePlacement {
        layout
            .nodes
            .iter()
            .find(|node| node.skill_id == skill_id)
            .expect("skill placement")
    }

    #[test]
    fn cross_job_dependencies_order_bands_left_to_right() {
        let tree = tree(&[
            (20, node(2, vec![(10, 3)])),
            (10, node(1, vec![])),
            (30, node(3, vec![(20, 2)])),
        ]);
        let names = HashMap::from([
            (1, "Novice".to_string()),
            (2, "Swordman".to_string()),
            (3, "Knight".to_string()),
        ]);

        let layout = TreeLayout::new(&tree, &names);

        assert_eq!(band_ids(&layout), vec![1, 2, 3]);
        assert!(layout.bands.windows(2).all(|pair| pair[0].x < pair[1].x));
    }

    #[test]
    fn disconnected_bands_use_names_then_ids_and_unresolved_ids() {
        let disconnected = tree(&[
            (40, node(4, vec![])),
            (30, node(3, vec![])),
            (20, node(2, vec![])),
            (10, node(1, vec![])),
        ]);
        let names = HashMap::from([
            (1, "Alpha".to_string()),
            (2, "Alpha".to_string()),
            (3, "Zeta".to_string()),
            (4, "Beta".to_string()),
        ]);

        assert_eq!(
            band_ids(&TreeLayout::new(&disconnected, &names)),
            vec![1, 2, 4, 3]
        );
        assert_eq!(
            band_ids(&TreeLayout::new(&disconnected, &HashMap::new())),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn same_job_depth_is_the_column_and_skill_id_orders_rows() {
        let tree = tree(&[
            (50, node(7, vec![])),
            (10, node(7, vec![])),
            (30, node(7, vec![(10, 1)])),
            (20, node(7, vec![(10, 1)])),
            (40, node(7, vec![(20, 1)])),
        ]);
        let layout = TreeLayout::new(&tree, &HashMap::new());

        assert_eq!(
            (placement(&layout, 10).column, placement(&layout, 10).row),
            (0, 0)
        );
        assert_eq!(
            (placement(&layout, 50).column, placement(&layout, 50).row),
            (0, 1)
        );
        assert_eq!(
            (placement(&layout, 20).column, placement(&layout, 20).row),
            (1, 0)
        );
        assert_eq!(
            (placement(&layout, 30).column, placement(&layout, 30).row),
            (1, 1)
        );
        assert_eq!(placement(&layout, 40).column, 2);
    }

    #[test]
    fn valid_requirements_have_identified_orthogonal_connectors() {
        let tree = tree(&[
            (10, node(1, vec![])),
            (20, node(1, vec![(10, 3)])),
            (30, node(2, vec![(20, 4)])),
        ]);
        let layout = TreeLayout::new(&tree, &HashMap::new());

        assert_eq!(layout.connectors.len(), 2);
        let same_job = layout
            .connectors
            .iter()
            .find(|connector| connector.target == 20)
            .expect("same-job connector");
        assert_eq!((same_job.source, same_job.minimum_level), (10, 3));
        assert!(!same_job.backlink);
        let cross_job = layout
            .connectors
            .iter()
            .find(|connector| connector.target == 30)
            .expect("cross-job connector");
        assert_eq!((cross_job.source, cross_job.minimum_level), (20, 4));
        assert!(!cross_job.backlink);
        for connector in &layout.connectors {
            assert_eq!(connector.segments[0].start.y, connector.segments[0].end.y);
            assert_eq!(connector.segments[1].start.x, connector.segments[1].end.x);
            assert_eq!(connector.segments[2].start.y, connector.segments[2].end.y);
        }
    }

    #[test]
    fn missing_prerequisites_do_not_create_nodes_or_connectors() {
        let tree = tree(&[(20, node(1, vec![(999, 5)]))]);
        let layout = TreeLayout::new(&tree, &HashMap::new());

        assert_eq!(layout.nodes.len(), 1);
        assert_eq!(layout.nodes[0].skill_id, 20);
        assert!(layout.connectors.is_empty());
        assert_eq!(layout.topology[0].requirements[0].skill_id, 999);
    }

    #[test]
    fn dependent_of_same_job_cycle_keeps_forward_depth() {
        let tree = tree(&[
            (10, node(1, vec![(20, 1)])),
            (20, node(1, vec![(10, 1)])),
            (30, node(1, vec![(20, 1)])),
        ]);
        let layout = TreeLayout::new(&tree, &HashMap::new());

        assert_eq!(
            placement(&layout, 30).column,
            placement(&layout, 20).column + 1
        );
        assert!(!placement(&layout, 30).cycle_fallback);
        let connector = layout
            .connectors
            .iter()
            .find(|connector| connector.source == 20 && connector.target == 30)
            .expect("cycle-to-dependent connector");
        assert!(!connector.backlink);
    }

    #[test]
    fn same_job_cycles_break_stably_and_mark_the_backlink() {
        let tree = tree(&[(10, node(1, vec![(20, 1)])), (20, node(1, vec![(10, 1)]))]);
        let layout = TreeLayout::new(&tree, &HashMap::new());

        assert!(layout.nodes.iter().all(|node| node.cycle_fallback));
        assert_eq!(placement(&layout, 20).column, 0);
        assert_eq!(placement(&layout, 10).column, 1);
        assert_eq!(
            layout
                .connectors
                .iter()
                .filter(|connector| connector.backlink)
                .count(),
            1
        );
    }

    #[test]
    fn cross_job_cycles_break_stably_and_expose_a_backlink() {
        let tree = tree(&[(10, node(1, vec![(20, 1)])), (20, node(2, vec![(10, 1)]))]);
        let names = HashMap::from([(1, "Zeta".to_string()), (2, "Alpha".to_string())]);
        let layout = TreeLayout::new(&tree, &names);

        assert_eq!(band_ids(&layout), vec![2, 1]);
        assert_eq!(
            layout.bands.iter().filter(|band| band.cycle_break).count(),
            1
        );
        assert_eq!(
            layout
                .connectors
                .iter()
                .filter(|edge| edge.backlink)
                .count(),
            1
        );
    }

    #[test]
    fn every_runtime_job_and_skill_appears_once() {
        let tree = tree(&[
            (10, node(1, vec![])),
            (20, node(1, vec![])),
            (30, node(2, vec![(10, 1)])),
        ]);
        let layout = TreeLayout::new(&tree, &HashMap::new());

        let jobs: std::collections::HashSet<_> =
            layout.bands.iter().map(|band| band.job_id).collect();
        let skills: std::collections::HashSet<_> =
            layout.nodes.iter().map(|node| node.skill_id).collect();
        assert_eq!(layout.bands.len(), jobs.len());
        assert_eq!(layout.nodes.len(), skills.len());
        assert_eq!(jobs, std::collections::HashSet::from([1, 2]));
        assert_eq!(skills, std::collections::HashSet::from([10, 20, 30]));
    }

    #[test]
    fn focus_contains_transitive_prerequisites_and_only_immediate_unlocks() {
        let tree = tree(&[
            (10, node(1, vec![])),
            (20, node(1, vec![(10, 1)])),
            (30, node(2, vec![(20, 2)])),
            (40, node(2, vec![(30, 1)])),
        ]);
        let focus = focus(&tree, 30).expect("focused runtime skill");

        assert_eq!(focus.focused, 30);
        assert_eq!(focus.prerequisite_nodes, [10, 20].into());
        assert_eq!(focus.prerequisite_edges, [(10, 20), (20, 30)].into());
        assert_eq!(focus.unlock_nodes, [40].into());
        assert_eq!(focus.unlock_edges, [(30, 40)].into());
    }

    #[test]
    fn focus_skips_missing_nodes_and_disconnected_branches() {
        let tree = tree(&[
            (10, node(1, vec![(999, 1)])),
            (20, node(1, vec![(10, 1)])),
            (70, node(3, vec![])),
        ]);
        let focused = focus(&tree, 20).expect("focused runtime skill");

        assert_eq!(focused.prerequisite_nodes, [10].into());
        assert_eq!(focused.prerequisite_edges, [(10, 20)].into());
        assert!(focused.unlock_nodes.is_empty());
        assert!(focus(&tree, 999).is_none());
    }

    #[test]
    fn focus_terminates_deterministically_on_cycles() {
        let tree = tree(&[
            (10, node(1, vec![(30, 1)])),
            (20, node(1, vec![(10, 1)])),
            (30, node(1, vec![(20, 1)])),
            (40, node(1, vec![(20, 1)])),
        ]);
        let first = focus(&tree, 20).expect("focused runtime skill");
        let second = focus(&tree, 20).expect("repeat focus");
        assert_eq!(first, second);
        assert_eq!(first.prerequisite_nodes, [10, 30].into());
        assert_eq!(
            first.prerequisite_edges,
            [(10, 20), (20, 30), (30, 10)].into()
        );
        assert_eq!(first.unlock_nodes, [30, 40].into());
        assert_eq!(first.unlock_edges, [(20, 30), (20, 40)].into());
    }

    #[test]
    fn empty_and_reordered_inputs_are_repeatable() {
        let empty = TreeLayout::new(&SkillTreeState::default(), &HashMap::new());
        assert!(empty.bands.is_empty());
        assert!(empty.nodes.is_empty());
        assert!(empty.connectors.is_empty());
        assert_eq!((empty.width, empty.height), (0.0, 0.0));

        let first = tree(&[
            (30, node(2, vec![(20, 2), (10, 1)])),
            (10, node(1, vec![])),
            (20, node(1, vec![])),
        ]);
        let second = tree(&[
            (20, node(1, vec![])),
            (10, node(1, vec![])),
            (30, node(2, vec![(10, 1), (20, 2)])),
        ]);
        assert_eq!(
            TreeLayout::new(&first, &HashMap::new()),
            TreeLayout::new(&second, &HashMap::new())
        );
    }
}
