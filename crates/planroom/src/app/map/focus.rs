//! `map --focus ID`: the blocks under one node [DR-85], which the pane's block rows and the
//! canvas's dive into a block read. The target is an ID, an ID's unique suffix or a unique name,
//! as every request's target is (contracts.md, the M1 leaves' requests).

use planroom_contract::{CommandName, Empty, EmptyReason, ErrorCode, Handle, Level, Node};
use planroom_kernel::tree;
use serde_json::json;

use crate::app::error::AppError;
use crate::text;

/// IDs a refusal names at most.
const CANDIDATES: usize = 5;

/// The node `target` names among `nodes`: the one with that ID, else the one whose ID ends with
/// `:target`, else the one named `target`. None is `not_found`, with the closest IDs; several is
/// `invalid_argument`, whose line names them.
pub(super) fn resolve<'a>(nodes: &'a [Node], target: &str) -> Result<&'a Node, AppError> {
    if let Some(node) = nodes.iter().find(|n| n.id.0 == target) {
        return Ok(node);
    }
    let suffix = format!(":{target}");
    let by_suffix: Vec<&Node> = nodes.iter().filter(|n| n.id.0.ends_with(&suffix)).collect();
    let by_name: Vec<&Node> = nodes.iter().filter(|n| n.name == target).collect();
    let found = if by_suffix.is_empty() { by_name } else { by_suffix };
    match found.as_slice() {
        [node] => Ok(node),
        [] => {
            let closest: Vec<&str> = nodes
                .iter()
                .filter(|n| n.id.0.contains(target) || n.name.contains(target))
                .take(CANDIDATES)
                .map(|n| n.id.0.as_str())
                .collect();
            let what = text::get("map.focus_missing", &[("target", &target)]);
            Err(AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": closest})))
        }
        several => {
            let ids: Vec<&str> = several.iter().take(CANDIDATES).map(|n| n.id.0.as_str()).collect();
            let what = text::get(
                "argument.focus_ambiguous",
                &[("target", &target), ("count", &several.len()), ("ids", &ids.join(", "))],
            );
            Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "focus"})))
        }
    }
}

/// The level a focus shows when the request names none: the one below the node's own, so a repo
/// opens on its containers and a container on its components. Code is the finest level, and the
/// intent level opens on the system below it.
pub(super) fn level_below(level: Level) -> Level {
    match level {
        Level::Intent => Level::System,
        Level::System => Level::Container,
        Level::Container => Level::Component,
        Level::Component | Level::Code => Level::Code,
    }
}

/// The empty state of a focus with nothing under it at `level`: its next action shows the node
/// itself, among the blocks beside it at its own level (under its parent, or at the top).
pub(super) fn nothing_under(node: &Node, level: Level, parent: Option<&Node>) -> Empty {
    let own = tree::level_name(node.level);
    let mut args = vec!["--level".to_owned(), own.to_owned()];
    if let Some(parent) = parent {
        args.extend(["--focus".to_owned(), parent.id.0.clone()]);
    }
    Empty {
        reason: EmptyReason("nothing_under".into()),
        line: Some(text::get(
            "empty.nothing_under",
            &[("name", &node.name), ("level", &tree::level_name(level))],
        )),
        next: Handle {
            command: CommandName("map".into()),
            args,
            label: text::get("empty.nothing_under_next", &[("name", &node.name)]),
        },
    }
}
