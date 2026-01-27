//! Tree view - main component that composes the proof tree visualization.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use super::{
    tree_given_bar::{render_conclusion, render_given_bar},
    tree_node_box::{node_height, render_node_box},
    ClickRegion, Selection,
};
use crate::tui_ipc::{NodeId, ProofDag};

/// Minimum width for a branch column.
const MIN_BRANCH_WIDTH: u16 = 25;

/// Context for recursive DAG-based tree rendering.
struct RenderContext<'a> {
    dag: &'a ProofDag,
    top_down: bool,
    selection: Option<Selection>,
    click_regions: &'a mut Vec<ClickRegion>,
}

/// Render the proof tree view from a `ProofDag`.
///
/// Works with both Paperproof-derived DAGs (rich data) and
/// local tactic-derived DAGs (basic structure only).
pub fn render_tree_view_from_dag(
    frame: &mut Frame,
    area: Rect,
    dag: &ProofDag,
    top_down: bool,
    selection: Option<Selection>,
    click_regions: &mut Vec<ClickRegion>,
) {
    if dag.is_empty() {
        frame.render_widget(
            Paragraph::new("No proof steps").style(Style::new().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let initial_hyps = &dag.initial_state.hypotheses;
    let initial_goal = dag
        .initial_state
        .goals
        .first()
        .map_or("", |g| g.type_.as_str());

    let (hyps_area, tree_area, conclusion_area) = layout_areas(area, top_down);

    click_regions.clear();

    render_given_bar(frame, hyps_area, initial_hyps, selection, click_regions);
    render_conclusion(
        frame,
        conclusion_area,
        initial_goal,
        selection,
        click_regions,
    );

    let mut ctx = RenderContext {
        dag,
        top_down,
        selection,
        click_regions,
    };

    if let Some(root_id) = dag.root {
        render_node_recursive(frame, tree_area, &mut ctx, root_id);
    }
}

/// Layout the three main areas: hypotheses, tree, conclusion.
fn layout_areas(area: Rect, top_down: bool) -> (Rect, Rect, Rect) {
    if top_down {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas::<3>(area)
        .into()
    } else {
        let [h, c, t] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .areas(area);
        (h, t, c)
    }
}

/// Recursively render a DAG node and its children.
fn render_node_recursive(frame: &mut Frame, area: Rect, ctx: &mut RenderContext, node_id: NodeId) {
    let Some(node) = ctx.dag.get(node_id) else {
        return;
    };

    let box_height = node_height(node);

    if area.height < box_height {
        return;
    }

    let (children_area, step_area) =
        layout_node_areas(area, box_height, node.children.is_empty(), ctx.top_down);

    render_node_box(frame, step_area, node, ctx.selection, ctx.click_regions);

    if let Some(children_area) = children_area {
        render_children(frame, children_area, ctx, &node.children);
    }
}

/// Layout step area and optional children area for a node.
fn layout_node_areas(
    area: Rect,
    box_height: u16,
    is_leaf: bool,
    top_down: bool,
) -> (Option<Rect>, Rect) {
    if is_leaf {
        if top_down {
            let [step_area, _] =
                Layout::vertical([Constraint::Length(box_height), Constraint::Fill(1)]).areas(area);
            (None, step_area)
        } else {
            let [_, step_area] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(box_height)]).areas(area);
            (None, step_area)
        }
    } else if top_down {
        let [step_area, children_area] =
            Layout::vertical([Constraint::Length(box_height), Constraint::Fill(1)]).areas(area);
        (Some(children_area), step_area)
    } else {
        let [children_area, step_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(box_height)]).areas(area);
        (Some(children_area), step_area)
    }
}

/// Render child branches from DAG.
fn render_children(frame: &mut Frame, area: Rect, ctx: &mut RenderContext, children: &[NodeId]) {
    if children.len() > 1 {
        let constraints: Vec<Constraint> = children
            .iter()
            .map(|_| Constraint::Min(MIN_BRANCH_WIDTH))
            .collect();

        let branch_areas = Layout::horizontal(constraints).split(area);

        for (&child_id, &branch_area) in children.iter().zip(branch_areas.iter()) {
            render_node_recursive(frame, branch_area, ctx, child_id);
        }
    } else if let Some(&child_id) = children.first() {
        render_node_recursive(frame, area, ctx, child_id);
    }
}
