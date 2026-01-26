//! Asynchronous goal fetching and broadcasting.

use std::sync::Arc;

use async_lsp::lsp_types::{Position, TextDocumentIdentifier};

use crate::{
    error::LspError,
    lean_rpc::{Goal, RpcClient},
    proxy::{
        documents::DocumentCache,
        tactic_finder::{
            find_case_splits, find_enclosing_definition, CaseSplitInfo, DefinitionInfo,
        },
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

        match fetch_combined_goals(&rpc, &text_document, position).await {
            Ok(goals) => {
                // Extract AST info (definition name and case splits)
                let (definition, case_splits) =
                    extract_ast_info(&doc_cache, uri.as_str(), position);

                socket_server.broadcast_goals(uri, position, goals, definition, case_splits);
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

fn extract_ast_info(
    doc_cache: &DocumentCache,
    uri: &str,
    position: Position,
) -> (Option<DefinitionInfo>, Vec<CaseSplitInfo>) {
    tracing::debug!("Looking up URI: {uri}");
    let Some((tree, source)) = doc_cache.get_tree_and_content(uri) else {
        tracing::warn!("No tree found for URI: {uri}");
        return (None, vec![]);
    };

    let definition = find_enclosing_definition(&tree, &source, position);
    let case_splits = find_case_splits(&tree, &source, position);

    tracing::debug!(
        "AST info at line {}: definition={:?}",
        position.line,
        definition.as_ref().map(|d| &d.name)
    );

    (definition, case_splits)
}
