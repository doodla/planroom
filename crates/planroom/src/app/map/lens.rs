//! A map through one lens: the relations it lifts and the kinds it is about, from the loaded
//! packs' vocabulary (architecture.md, "Lenses and questions"; DR-15, DR-46).
//!
//! With no `--lens`, a map is the default lens's, the code pack's `runtime`; `--lens all` names
//! no lens and is about every kind of every loaded pack, lifting every relation. A lens a request
//! names that no loaded pack declares is `invalid_argument`. A lens with kinds that leaves no
//! block at a level shows the empty state `lens_shows_nothing`, whose next action is the same
//! level through `--lens all` (DR-21: no screen is blank).

use planroom_contract::{
    CommandName, Empty, EmptyReason, ErrorCode, Handle, Kind, Level, MapView, Vocabulary,
};
use planroom_kernel::{lens, tree};
use serde_json::json;

use crate::app::error::AppError;
use crate::text;

/// `--lens all`: every kind and every relation of every loaded pack. It names no lens, so no pack
/// may declare a lens with this ID.
pub const ALL: &str = "all";

/// The default lens's ID, or `all` when no loaded pack declares a default.
pub fn default_id(vocabulary: &Vocabulary) -> String {
    lens::default(vocabulary).map_or_else(|| ALL.to_owned(), |l| l.id.clone())
}

/// The lens a request asks for, checked: the one it names, or the default with none.
pub fn asked(vocabulary: &Vocabulary, asked: Option<&str>) -> Result<String, AppError> {
    match asked {
        None => Ok(default_id(vocabulary)),
        Some(ALL) => Ok(ALL.to_owned()),
        Some(id) if lens::named(vocabulary, id).is_some() => Ok(id.to_owned()),
        Some(id) => {
            let known: Vec<&str> =
                std::iter::once(ALL).chain(vocabulary.lenses.iter().map(|l| l.id.as_str())).collect();
            let what = text::get("argument.unknown_lens", &[("lens", &id), ("known", &known.join(", "))]);
            Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "lens", "lenses": known})))
        }
    }
}

/// The level `--lens X` alone takes: the lens's own, for a lens the request names. The default
/// lens and `all` keep the top of the map.
pub fn level_of(vocabulary: &Vocabulary, asked: Option<&str>) -> Option<Level> {
    lens::named(vocabulary, asked?).map(|l| l.level)
}

/// The kinds the lens `lens` is about, whose blocks come first in a map's cut order and display
/// order; none for `all`, a lens whose `kinds` is empty, or a lens no loaded pack declares.
pub fn kinds<'a>(vocabulary: &'a Vocabulary, lens: &str) -> &'a [Kind] {
    lens::named(vocabulary, lens).map_or(&[], |l| l.kinds.as_slice())
}

/// `view` through the lens `lens`: its relations' edges; with kinds, only their blocks and the
/// blocks those edges touch, each with its parent chain, its kinds' blocks first; and every
/// block's tone. `all`, or a lens no loaded pack declares, keeps every block and edge. A lens
/// with kinds that leaves none of the view's blocks gives the empty state `lens_shows_nothing`.
pub fn filter(view: MapView, vocabulary: &Vocabulary, lens: &str) -> MapView {
    let had = !view.nodes.is_empty();
    let mut view = lens::apply(view, vocabulary, lens::named(vocabulary, lens));
    if had && view.nodes.is_empty() && view.empty.is_none() {
        view.empty = Some(shows_nothing(lens, &view));
    }
    view
}

/// The empty state of a lens that leaves no block in `view`: the line names the lens and the
/// level, and the next action shows the same blocks through `--lens all`.
fn shows_nothing(lens: &str, view: &MapView) -> Empty {
    let level = tree::level_name(view.level);
    let mut args = vec!["--level".to_owned(), level.to_owned(), "--lens".to_owned(), ALL.to_owned()];
    if let Some(focus) = &view.focus {
        args.extend(["--focus".to_owned(), focus.0.clone()]);
    }
    Empty {
        reason: EmptyReason("lens_shows_nothing".into()),
        line: Some(text::get("empty.lens_shows_nothing", &[("lens", &lens), ("level", &level)])),
        next: Handle {
            command: CommandName("map".into()),
            args,
            label: text::label("empty.lens_shows_nothing_next", &[]),
        },
    }
}
