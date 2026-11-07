use std::sync::Arc;

use hoarddeck_domain::ModelId;

use crate::ports::ModelRepository;
use crate::{AppError, ModelDetail};

pub struct GetModel {
    pub models: Arc<dyn ModelRepository>,
}

impl GetModel {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn execute(&self, id: ModelId) -> Result<Option<ModelDetail>, AppError> {
        Ok(self.models.get(id)?)
    }
}
