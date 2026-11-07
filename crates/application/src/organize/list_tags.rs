use std::sync::Arc;

use hoarddeck_domain::{LibraryId, TagName};

use crate::AppError;
use crate::ports::MetaStore;

/// Tags in use in a library, most used first, for the sidebar and tag suggestions.
pub struct ListTags {
    pub meta: Arc<dyn MetaStore>,
}

impl ListTags {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn execute(&self, library: LibraryId) -> Result<Vec<(TagName, u32)>, AppError> {
        Ok(self.meta.tags(library)?)
    }
}
