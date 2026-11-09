use std::sync::{Arc, PoisonError, RwLock};

use super::{CompiledTagRules, TagRule};
use crate::AppError;
use crate::ports::TagRuleStore;

/// The saved tag rules, compiled once and shared by everything that regroups models.
pub struct TagRuleBook {
    store: Arc<dyn TagRuleStore>,
    compiled: RwLock<Arc<CompiledTagRules>>,
}

impl TagRuleBook {
    /// Loads the saved rules. Rules that no longer compile (an older app wrote them) are reported and left out, so the
    /// app still opens.
    #[must_use]
    pub fn new(store: Arc<dyn TagRuleStore>) -> Self {
        let compiled = store
            .load_tag_rules()
            .map_err(AppError::from)
            .and_then(|rules| CompiledTagRules::compile(&rules))
            .unwrap_or_else(|error| {
                eprintln!("tag rules not used: {error}");
                CompiledTagRules::default()
            });
        Self { store, compiled: RwLock::new(Arc::new(compiled)) }
    }

    #[must_use]
    pub fn current(&self) -> Arc<CompiledTagRules> {
        Arc::clone(&self.compiled.read().unwrap_or_else(PoisonError::into_inner))
    }

    pub(crate) fn rules(&self) -> Result<Vec<TagRule>, AppError> {
        Ok(self.store.load_tag_rules()?)
    }

    pub(crate) fn save(&self, rules: &[TagRule]) -> Result<(), AppError> {
        let compiled = CompiledTagRules::compile(rules)?;
        self.store.save_tag_rules(rules)?;
        *self.compiled.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(compiled);
        Ok(())
    }
}
