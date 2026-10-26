//! A map through one lens: the relations it lifts and the kinds it is about, from the loaded
//! packs' vocabulary (architecture.md, "Lenses and questions").

use planroom_contract::{MapView, Vocabulary};

/// `view` through the lens `lens`. No pack declares a lens yet, so every lens keeps the whole view.
pub fn filter(view: MapView, vocabulary: &Vocabulary, lens: &str) -> MapView {
    let _ = (vocabulary, lens);
    view
}
