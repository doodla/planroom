//! `map --focus ID`: the map centred on one node. Until it lands, a focus is refused with
//! `invalid_argument`.

use planroom_contract::MapReq;

use crate::app::error::AppError;

/// Refuse a request that asks for a focus, which `map` doesn't serve yet.
pub(super) fn check(req: &MapReq) -> Result<(), AppError> {
    if req.focus.is_some() {
        return Err(AppError::invalid_argument("focus", "argument.focus_unsupported"));
    }
    Ok(())
}
