//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier};

use crate::{
    error::LspError,
    lean_rpc::{fetch_paperproof_via_cli, Goal, PaperproofMode, PaperproofStep, RpcClient},
    proxy::{
        ast::{find_all_tactics_in_proof, find_enclosing_definition, DefinitionInfo, TacticInfo},
        dag::{ProofDag, ProofDagSource},
        lsp::DocumentCache,
    },
    tui_ipc::{CursorInfo, SocketServer},
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
                // Extract AST info (definition name and tactics)
                let (definition, local_tactics) =
                    extract_ast_info(&doc_cache, uri.as_str(), position);

                // Build ProofDag: prefer Paperproof, fallback to local tactics
                let mut proof_dag = build_proof_dag(
                    paperproof_result.as_ref(),
                    &local_tactics,
                    &goals,
                    position,
                    definition.as_ref().map(|d| d.name.clone()),
                );

                // Resolve goto locations for Paperproof-based DAGs
                // (local tactics DAGs already have locations from the goals)
                resolve_dag_goto_locations(&mut proof_dag, &rpc, &text_document).await;

                socket_server.broadcast_goals(uri, position, goals, definition, proof_dag);
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
///
/// Tries RPC first (fast path when `import Paperproof` is present),
/// falls back to CLI when RPC is unavailable (slower, but works with just lakefile dep).
async fn fetch_paperproof_steps(
    rpc_client: &RpcClient,
    text_document: &TextDocumentIdentifier,
    position: Position,
) -> Option<Vec<PaperproofStep>> {
    // Try RPC first (fast path)
    match rpc_client
        .get_paperproof_snapshot(text_document, position, PaperproofMode::Tree)
        .await
    {
        Ok(Some(output)) if !output.steps.is_empty() => {
            return Some(output.steps);
        }
        Ok(Some(_)) => {
            // RPC returned empty steps, try CLI fallback
            tracing::debug!("Paperproof RPC returned empty steps, trying CLI fallback");
        }
        Ok(None) => {
            // RPC returned None - method not available, try CLI fallback
            tracing::debug!("Paperproof RPC unavailable, trying CLI fallback");
        }
        Err(e) => {
            tracing::debug!("Paperproof RPC failed: {e}, trying CLI fallback");
        }
    }

    // CLI fallback: extract file path from URI
    let file_path = text_document.uri.path().to_string();

    match fetch_paperproof_via_cli(
        &file_path,
        position.line,
        position.character,
        PaperproofMode::Tree,
    )
    .await
    {
        Some(output) if !output.steps.is_empty() => {
            tracing::debug!("Paperproof CLI returned {} steps", output.steps.len());
            Some(output.steps)
        }
        Some(_) => {
            tracing::debug!("Paperproof CLI returned empty steps");
            None
        }
        None => {
            tracing::debug!("Paperproof CLI not available");
            None
        }
    }
}

fn extract_ast_info(
    doc_cache: &DocumentCache,
    uri: &str,
    position: Position,
) -> (Option<DefinitionInfo>, Vec<TacticInfo>) {
    tracing::debug!("Looking up URI: {uri}");
    let Some((tree, source)) = doc_cache.get_tree_and_content(uri) else {
        tracing::warn!("No tree found for URI: {uri}");
        return (None, vec![]);
    };

    let definition = find_enclosing_definition(&tree, &source, position);
    let tactics = find_all_tactics_in_proof(&tree, &source, position);

    tracing::debug!(
        "AST info at line {}: definition={:?}, tactics={}",
        position.line,
        definition.as_ref().map(|d| &d.name),
        tactics.len()
    );

    (definition, tactics)
}

/// Build a `ProofDag` from available data.
///
/// Prefers Paperproof data when available (richer information),
/// falls back to local tree-sitter tactics otherwise.
fn build_proof_dag(
    paperproof_steps: Option<&Vec<PaperproofStep>>,
    local_tactics: &[TacticInfo],
    goals: &[Goal],
    cursor_position: Position,
    definition_name: Option<String>,
) -> Option<ProofDag> {
    // Prefer Paperproof data when available
    if let Some(steps) = paperproof_steps {
        if !steps.is_empty() {
            return Some(ProofDag::from_paperproof_steps(
                steps,
                cursor_position,
                definition_name,
            ));
        }
    }

    // Fall back to local tactics
    if !local_tactics.is_empty() {
        return Some(ProofDag::from_local_tactics(
            local_tactics,
            goals,
            cursor_position,
            definition_name,
        ));
    }

    None
}

/// Resolve goto locations for Paperproof-based DAGs.
async fn resolve_dag_goto_locations(
    proof_dag: &mut Option<ProofDag>,
    rpc: &RpcClient,
    text_document: &TextDocumentIdentifier,
) {
    let Some(dag) = proof_dag
        .as_mut()
        .filter(|d| d.source == ProofDagSource::Paperproof)
    else {
        return;
    };
    dag.resolve_goto_locations(rpc, text_document).await;
}
