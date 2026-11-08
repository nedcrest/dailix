//! Opening an edit in Omnix (Omnix's `sibling` finds and starts it).

use std::path::Path;

/// Opens `edit` in Omnix; what went wrong, said plainly.
pub fn open(edit: &Path) -> Result<(), String> {
    omnix::sibling::open_in_omnix(edit)
}
