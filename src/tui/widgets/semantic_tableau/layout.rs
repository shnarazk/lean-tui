//! Layout for the semantic tableau - combines given, proof, and theorem panes.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::StatefulWidget,
};

use super::{
    given_pane::{GivenPane, GivenPaneState},
    proof_pane::{ProofPane, ProofPaneState},
    theorem_pane::{TheoremPane, TheoremPaneState},
    Selection,
};
use crate::lean_rpc::{ProofDag, ProofState};

/// Combined state for the semantic tableau layout.
#[derive(Default)]
pub struct SemanticTableauState {
    /// State for the given pane.
    pub given: GivenPaneState,
    /// State for the proof pane.
    pub proof: ProofPaneState,
    /// State for the theorem pane.
    pub theorem: TheoremPaneState,
}

impl SemanticTableauState {
    /// Find click at position across all panes.
    pub fn find_click_at(&self, x: u16, y: u16) -> Option<Selection> {
        self.given
            .find_click_at(x, y)
            .or_else(|| self.proof.find_click_at(x, y))
            .or_else(|| self.theorem.find_click_at(x, y))
    }

    /// Update state when current node changes.
    pub fn update_current_node(&mut self, current_node: Option<u32>) {
        self.proof.update_current_node(current_node);
    }
}

/// Semantic tableau layout widget - combines given, proof, and theorem panes.
pub struct SemanticTableauLayout<'a> {
    dag: &'a ProofDag,
    top_down: bool,
    selection: Option<Selection>,
    /// Current proof state from LSP (may differ from node's `state_after`).
    current_state: &'a ProofState,
}

impl<'a> SemanticTableauLayout<'a> {
    pub const fn new(
        dag: &'a ProofDag,
        top_down: bool,
        selection: Option<Selection>,
        current_state: &'a ProofState,
    ) -> Self {
        Self {
            dag,
            top_down,
            selection,
            current_state,
        }
    }
}

impl StatefulWidget for SemanticTableauLayout<'_> {
    type State = SemanticTableauState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let (given_area, proof_area, theorem_area) = compute_layout(area, self.top_down);

        // Render given pane
        let given_widget = GivenPane::new(&self.dag.initial_state.hypotheses, self.selection);
        given_widget.render(given_area, buf, &mut state.given);

        // Render proof pane with actual current state
        let proof_widget =
            ProofPane::new(self.dag, self.top_down, self.selection, self.current_state);
        proof_widget.render(proof_area, buf, &mut state.proof);

        // Render theorem pane with the top-level theorem (initial goal)
        let theorem_goal = self
            .dag
            .initial_state
            .goals
            .first()
            .map(|g| g.type_.to_plain_text())
            .unwrap_or_default();
        let theorem_widget = TheoremPane::new(&theorem_goal, self.selection);
        theorem_widget.render(theorem_area, buf, &mut state.theorem);
    }
}

/// Compute layout areas for the three panes.
fn compute_layout(area: Rect, top_down: bool) -> (Rect, Rect, Rect) {
    if top_down {
        // Top-down: Given at top, Proof in middle, Theorem at bottom
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas::<3>(area)
        .into()
    } else {
        // Bottom-up: Given at top, Theorem below it, Proof at bottom
        let [given, theorem, proof] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .areas(area);
        (given, proof, theorem)
    }
}
