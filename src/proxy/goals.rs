//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier};

use crate::{
    error::LspError,
    lean_rpc::{Goal, PaperproofMode, PaperproofStep, RpcClient},
    proxy::{
        ast::{
            find_all_tactics_in_proof, find_case_splits, find_enclosing_definition, CaseSplitInfo,
            DefinitionInfo, TacticInfo,
        },
        lsp::DocumentCache,
    },
    tui_ipc::{CursorInfo, ProofStep, SocketServer},
};

pub async fn fetch_combined_goals(
    rpc_client: &RpcClient,
    text_document: &TextDocumentIdentifier,
    position: Position,
) -> Result<Vec<Goal>, LspError> {
    let (tactic_result, term_result) = tokio::join!(
        rpc_client.get_goals(text_document, position),
        rpc_client.get_term_goal(text_document, position)
    );

    let term_goal = term_result.ok().flatten();
    let tactic_goals = match tactic_result {
        Ok(goals) => goals,
        Err(e) if term_goal.is_none() => return Err(e),
        Err(_) => vec![],
    };

    let mut goals: Vec<Goal> = term_goal.into_iter().chain(tactic_goals).collect();

    // Pre-resolve goto locations while RPC session is active
    for goal in &mut goals {
        rpc_client
            .resolve_goto_locations(text_document, position, goal)
            .await;
    }

    Ok(goals)
}

pub fn spawn_goal_fetch(
    cursor: &CursorInfo,
    socket_server: &Arc<SocketServer>,
    rpc_client: &Arc<RpcClient>,
    doc_cache: &Arc<DocumentCache>,
) {
    let rpc = rpc_client.clone();
    let socket_server = socket_server.clone();
    let doc_cache = doc_cache.clone();
    let uri = cursor.uri.clone();
    let position = cursor.position;

    tokio::spawn(async move {
        let text_document = TextDocumentIdentifier::new(uri.clone());

        // Fetch goals and Paperproof data in parallel
        let (goals_result, paperproof_result) = tokio::join!(
            fetch_combined_goals(&rpc, &text_document, position),
            fetch_paperproof_steps(&rpc, &text_document, position)
        );

        match goals_result {
            Ok(goals) => {
                // Extract AST info (definition name, case splits, and tactics)
                let (definition, case_splits, local_tactics) =
                    extract_ast_info(&doc_cache, uri.as_str(), position);

                // Build unified proof steps: prefer Paperproof if available, else use local
                let (proof_steps, current_step_index) =
                    build_proof_steps(paperproof_result.as_ref(), &local_tactics, position);

                socket_server.broadcast_goals(
                    uri,
                    position,
                    goals,
                    definition,
                    case_splits,
                    paperproof_result,
                    proof_steps,
                    current_step_index,
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Could not fetch goals at {uri}:{}:{}: {e}",
                    position.line,
                    position.character
                );
                socket_server.broadcast_error(e.to_string());
            }
        }
    });
}

/// Fetch Paperproof proof steps if available.
async fn fetch_paperproof_steps(
    rpc_client: &RpcClient,
    text_document: &TextDocumentIdentifier,
    position: Position,
) -> Option<Vec<PaperproofStep>> {
    match rpc_client
        .get_paperproof_snapshot(text_document, position, PaperproofMode::Tree)
        .await
    {
        Ok(Some(output)) => {
            if output.steps.is_empty() {
                None
            } else {
                Some(output.steps)
            }
        }
        Ok(None) => None,
        Err(e) => {
            tracing::debug!("Paperproof fetch failed: {e}");
            None
        }
    }
}

fn extract_ast_info(
    doc_cache: &DocumentCache,
    uri: &str,
    position: Position,
) -> (Option<DefinitionInfo>, Vec<CaseSplitInfo>, Vec<TacticInfo>) {
    tracing::debug!("Looking up URI: {uri}");
    let Some((tree, source)) = doc_cache.get_tree_and_content(uri) else {
        tracing::warn!("No tree found for URI: {uri}");
        return (None, vec![], vec![]);
    };

    let definition = find_enclosing_definition(&tree, &source, position);
    let case_splits = find_case_splits(&tree, &source, position);
    let tactics = find_all_tactics_in_proof(&tree, &source, position);

    tracing::debug!(
        "AST info at line {}: definition={:?}, tactics={}",
        position.line,
        definition.as_ref().map(|d| &d.name),
        tactics.len()
    );

    (definition, case_splits, tactics)
}

/// Build unified proof steps from either Paperproof or local tactics.
fn build_proof_steps(
    paperproof_steps: Option<&Vec<PaperproofStep>>,
    local_tactics: &[TacticInfo],
    cursor_position: Position,
) -> (Vec<ProofStep>, usize) {
    // Prefer Paperproof data if available
    if let Some(steps) = paperproof_steps {
        if !steps.is_empty() {
            let proof_steps: Vec<ProofStep> =
                steps.iter().map(ProofStep::from_paperproof).collect();
            let current_index = find_current_step_index(&proof_steps, cursor_position);
            return (proof_steps, current_index);
        }
    }

    // Fall back to local tactics
    if local_tactics.is_empty() {
        return (vec![], 0);
    }

    let proof_steps: Vec<ProofStep> = local_tactics
        .iter()
        .map(|t| {
            let depends_on = extract_dependencies(&t.text);
            ProofStep::from_local(t, depends_on)
        })
        .collect();

    let current_index = find_current_step_index(&proof_steps, cursor_position);
    (proof_steps, current_index)
}

/// Extract hypothesis names that appear in the tactic text.
fn extract_dependencies(tactic_text: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let words: Vec<&str> = tactic_text.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        if i == 0 {
            continue;
        }

        let clean = word.trim_matches(|c| c == '[' || c == ']' || c == ',' || c == '⟨' || c == '⟩');

        if clean.is_empty()
            || clean.starts_with('-')
            || clean.starts_with('*')
            || clean == "with"
            || clean == "at"
            || clean == "only"
        {
            continue;
        }

        if clean.chars().next().is_some_and(char::is_lowercase) {
            deps.push(clean.to_string());
        }
    }

    deps
}

/// Find the index of the step closest to the cursor position.
fn find_current_step_index(steps: &[ProofStep], cursor: Position) -> usize {
    let mut best_index = 0;
    let mut best_distance = i64::MAX;

    for (i, step) in steps.iter().enumerate() {
        let line_diff = (i64::from(step.position.line) - i64::from(cursor.line)).abs();
        let char_diff = (i64::from(step.position.character) - i64::from(cursor.character)).abs();
        let distance = line_diff * 1000 + char_diff;

        // Prefer steps at or before cursor
        let penalty = if step.position.line > cursor.line {
            10000
        } else {
            0
        };

        if distance + penalty < best_distance {
            best_distance = distance + penalty;
            best_index = i;
        }
    }

    best_index
}
