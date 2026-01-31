//! Navigation for the semantic tableau view.
//!
//! Handles keyboard navigation through the proof tree using spatial regions.

use tracing::debug;

use super::{tree_layout::TreeLayout, Selection};
use crate::lean_rpc::ProofDag;

/// Region for keyboard navigation - uses virtual coordinates (i32).
#[derive(Debug, Clone, Copy)]
pub struct NavigationRegion {
    /// Virtual canvas position - can be negative or outside viewport.
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
    /// The selectable item at this position.
    pub selection: Selection,
}

/// Direction for spatial navigation.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// Check if `other` is ahead of `cur` in this direction.
    pub fn is_ahead(self, cur: &NavigationRegion, other: &NavigationRegion) -> bool {
        match self {
            Self::Left => other.x + i32::from(other.width) <= cur.x,
            Self::Right => other.x >= cur.x + i32::from(cur.width),
            Self::Up => other.y + i32::from(other.height) <= cur.y,
            Self::Down => other.y >= cur.y + i32::from(cur.height),
        }
    }

    /// Calculate distance from `cur` to `other` in this direction.
    pub fn distance(self, cur: &NavigationRegion, other: &NavigationRegion) -> i32 {
        match self {
            Self::Left => (cur.x - (other.x + i32::from(other.width))).max(0),
            Self::Right => (other.x - (cur.x + i32::from(cur.width))).max(0),
            Self::Up => (cur.y - (other.y + i32::from(other.height))).max(0),
            Self::Down => (other.y - (cur.y + i32::from(cur.height))).max(0),
        }
    }

    /// Whether this is a horizontal direction.
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// Check if two 1D ranges overlap.
fn ranges_overlap(a_start: i32, a_len: u16, b_start: i32, b_len: u16) -> bool {
    let a_end = a_start + i32::from(a_len);
    let b_end = b_start + i32::from(b_len);
    a_start < b_end && b_start < a_end
}

/// Build navigation regions for tree nodes only.
///
/// These regions use virtual coordinates and are used for keyboard navigation,
/// allowing navigation to items outside the current viewport.
///
/// Given bar hypotheses and Theorem are excluded from keyboard navigation
/// (they remain mouse-clickable) to avoid coordinate system complexity
/// between screen coordinates (Given/Theorem) and virtual coordinates (tree
/// nodes).
#[allow(clippy::cast_possible_wrap)]
pub fn build_navigation_regions(dag: &ProofDag, layout: &TreeLayout) -> Vec<NavigationRegion> {
    let mut regions = Vec::new();

    debug!(
        node_count = layout.nodes.len(),
        "Building navigation regions"
    );

    // Only node goals and hypotheses - use virtual coordinates from layout
    for pos in &layout.nodes {
        let Some(node) = dag.get(pos.node_id) else {
            continue;
        };

        debug!(
            node_id = pos.node_id,
            x = pos.x,
            y = pos.y,
            w = pos.width,
            h = pos.height,
            new_hyps = node.new_hypotheses.len(),
            goals = node.state_after.goals.len(),
            "Processing node for navigation"
        );

        // New hypotheses in this node
        for (i, &hyp_idx) in node.new_hypotheses.iter().enumerate() {
            let hyp_width = 15u16;
            let hyp_offset = (i as i32) * (i32::from(hyp_width) + 1);
            let region = NavigationRegion {
                x: pos.x + hyp_offset,
                y: pos.y,
                width: hyp_width,
                height: 1,
                selection: Selection::Hyp {
                    node_id: pos.node_id,
                    hyp_idx,
                },
            };
            debug!(
                i,
                hyp_idx,
                x = region.x,
                y = region.y,
                "Adding hypothesis navigation region"
            );
            regions.push(region);
        }

        // Goals in this node - offset each goal horizontally for navigation
        let goal_count = node.state_after.goals.len();
        let goal_width = if goal_count > 1 {
            // Divide available width among goals
            (pos.width.saturating_sub(2) / goal_count as u16).max(15)
        } else {
            pos.width.saturating_sub(2)
        };

        for (i, _) in node.state_after.goals.iter().enumerate() {
            let goal_offset = (i as i32) * (i32::from(goal_width) + 1);
            let region = NavigationRegion {
                x: pos.x + 1 + goal_offset,
                y: pos.y + i32::from(pos.height.saturating_sub(2)),
                width: goal_width,
                height: 1,
                selection: Selection::Goal {
                    node_id: pos.node_id,
                    goal_idx: i,
                },
            };
            debug!(
                goal_idx = i,
                x = region.x,
                y = region.y,
                "Adding goal navigation region"
            );
            regions.push(region);
        }
    }

    debug!(total_regions = regions.len(), "Navigation regions built");
    regions
}

/// Find the nearest selectable item in the given direction using grid-based
/// navigation.
pub fn find_nearest_in_direction(
    regions: &[NavigationRegion],
    current: Selection,
    direction: Direction,
) -> Option<Selection> {
    debug!(
        ?current,
        ?direction,
        regions_count = regions.len(),
        "Navigation request"
    );

    let Some(cur) = regions.iter().find(|r| r.selection == current) else {
        debug!("Current selection not found in navigation regions");
        return None;
    };

    debug!(
        x = cur.x,
        y = cur.y,
        w = cur.width,
        h = cur.height,
        "Found current region"
    );

    // Grid-aligned search: only items that overlap on the perpendicular axis
    let aligned: Vec<_> = regions
        .iter()
        .filter(|r| {
            let overlaps = if direction.is_horizontal() {
                ranges_overlap(cur.y, cur.height, r.y, r.height)
            } else {
                ranges_overlap(cur.x, cur.width, r.x, r.width)
            };
            overlaps && direction.is_ahead(cur, r)
        })
        .collect();

    debug!(aligned_count = aligned.len(), "Aligned candidates");
    for (i, r) in aligned.iter().enumerate() {
        debug!(
            i,
            x = r.x,
            y = r.y,
            dist = direction.distance(cur, r),
            ?r.selection,
            "Aligned candidate"
        );
    }

    let result = aligned
        .into_iter()
        .min_by_key(|r| direction.distance(cur, r))
        .map(|r| r.selection);

    debug!(?result, "Navigation result");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test navigation between sibling hypotheses in the same node.
    #[test]
    fn test_sibling_hypothesis_navigation() {
        // Simulate two hypotheses in the same node at the same y position
        let regions = vec![
            NavigationRegion {
                x: 0,
                y: 10,
                width: 15,
                height: 1,
                selection: Selection::Hyp {
                    node_id: 1,
                    hyp_idx: 0,
                },
            },
            NavigationRegion {
                x: 16,
                y: 10,
                width: 15,
                height: 1,
                selection: Selection::Hyp {
                    node_id: 1,
                    hyp_idx: 1,
                },
            },
            // Goal below the hypotheses
            NavigationRegion {
                x: 1,
                y: 12,
                width: 30,
                height: 1,
                selection: Selection::Goal {
                    node_id: 1,
                    goal_idx: 0,
                },
            },
        ];

        let result = find_nearest_in_direction(
            &regions,
            Selection::Hyp {
                node_id: 1,
                hyp_idx: 0,
            },
            Direction::Right,
        );
        assert_eq!(
            result,
            Some(Selection::Hyp {
                node_id: 1,
                hyp_idx: 1
            }),
            "Right from first hyp should go to second hyp"
        );

        let result = find_nearest_in_direction(
            &regions,
            Selection::Hyp {
                node_id: 1,
                hyp_idx: 1,
            },
            Direction::Left,
        );
        assert_eq!(
            result,
            Some(Selection::Hyp {
                node_id: 1,
                hyp_idx: 0
            }),
            "Left from second hyp should go to first hyp"
        );

        let result = find_nearest_in_direction(
            &regions,
            Selection::Hyp {
                node_id: 1,
                hyp_idx: 0,
            },
            Direction::Down,
        );
        assert_eq!(
            result,
            Some(Selection::Goal {
                node_id: 1,
                goal_idx: 0
            }),
            "Down from hyp should go to goal"
        );

        let result = find_nearest_in_direction(
            &regions,
            Selection::Goal {
                node_id: 1,
                goal_idx: 0,
            },
            Direction::Up,
        );
        assert!(
            matches!(
                result,
                Some(Selection::Hyp {
                    node_id: 1,
                    hyp_idx: 0 | 1
                })
            ),
            "Up from goal should go to a hypothesis"
        );
    }

    /// Test that navigation returns None when no target exists in direction.
    #[test]
    fn test_navigation_no_target() {
        let regions = vec![NavigationRegion {
            x: 0,
            y: 10,
            width: 15,
            height: 1,
            selection: Selection::Hyp {
                node_id: 1,
                hyp_idx: 0,
            },
        }];

        // No target to the right
        let result = find_nearest_in_direction(
            &regions,
            Selection::Hyp {
                node_id: 1,
                hyp_idx: 0,
            },
            Direction::Right,
        );
        assert_eq!(result, None, "No target to the right");

        // No target to the left
        let result = find_nearest_in_direction(
            &regions,
            Selection::Hyp {
                node_id: 1,
                hyp_idx: 0,
            },
            Direction::Left,
        );
        assert_eq!(result, None, "No target to the left");
    }

    #[test]
    fn test_ranges_overlap() {
        // Same position
        assert!(ranges_overlap(10, 1, 10, 1));
        // Adjacent (no overlap)
        assert!(!ranges_overlap(10, 1, 11, 1));
        // Overlapping
        assert!(ranges_overlap(10, 5, 12, 5));
        // One contains the other
        assert!(ranges_overlap(10, 10, 12, 2));
    }

    /// Test navigation between multiple goals in the same node.
    #[test]
    fn test_sibling_goals_navigation() {
        // Simulate two goals in the same node at the same y position but different x
        let regions = vec![
            NavigationRegion {
                x: 1,
                y: 5,
                width: 24,
                height: 1,
                selection: Selection::Goal {
                    node_id: 1,
                    goal_idx: 0,
                },
            },
            NavigationRegion {
                x: 26,
                y: 5,
                width: 24,
                height: 1,
                selection: Selection::Goal {
                    node_id: 1,
                    goal_idx: 1,
                },
            },
        ];

        // From goal 0, pressing Right should go to goal 1
        let result = find_nearest_in_direction(
            &regions,
            Selection::Goal {
                node_id: 1,
                goal_idx: 0,
            },
            Direction::Right,
        );
        assert_eq!(
            result,
            Some(Selection::Goal {
                node_id: 1,
                goal_idx: 1
            }),
            "Right from first goal should go to second goal"
        );

        // From goal 1, pressing Left should go to goal 0
        let result = find_nearest_in_direction(
            &regions,
            Selection::Goal {
                node_id: 1,
                goal_idx: 1,
            },
            Direction::Left,
        );
        assert_eq!(
            result,
            Some(Selection::Goal {
                node_id: 1,
                goal_idx: 0
            }),
            "Left from second goal should go to first goal"
        );
    }
}
