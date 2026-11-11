use std::sync::Arc;

use hoarddeck_domain::{Collection, CollectionId, ModelId, SmartSearch, collection_name};

use crate::ports::{CollectionStore, ModelRepository};
use crate::search::{SearchModels, SearchRequest};
use crate::{AppError, ModelSummary, Refusal};

/// Hand-made collections keep the order the user gives them; smart ones are a saved search with its scope.
pub struct ManageCollections {
    pub store: Arc<dyn CollectionStore>,
    pub models: Arc<dyn ModelRepository>,
    pub search: Arc<SearchModels>,
    pub sync: Arc<crate::sidecar::SidecarSync>,
}

impl ManageCollections {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn list(&self) -> Result<Vec<(Collection, Option<u32>)>, AppError> {
        Ok(self.store.list_collections()?)
    }

    /// `smart`: the saved search for a smart collection, `None` for a hand-made one.
    /// # Errors
    /// Fails for an invalid name or when the index cannot be written.
    pub fn create(&self, name: &str, smart: Option<SmartSearch>) -> Result<Collection, AppError> {
        let smart = smart.filter(|search| !search.text.trim().is_empty());
        if smart.as_ref().is_some_and(|search| search.scope.is_some() && search.library.is_none()) {
            return Err(AppError::InvalidInput("a folder scope needs a library".to_owned()));
        }
        let done = self.store.create_collection(&collection_name(name)?, smart.as_ref())?;
        // Collections can span libraries; each library file keeps its part.
        self.sync.all_libraries_changed();
        Ok(done)
    }

    /// # Errors
    /// Fails for an invalid name or when the index cannot be written.
    pub fn rename(&self, id: CollectionId, name: &str) -> Result<(), AppError> {
        self.store.rename_collection(id, &collection_name(name)?)?;
        // Collections can span libraries; each library file keeps its part.
        self.sync.all_libraries_changed();
        Ok(())
    }

    /// Deletes the collection only; its models stay in the library.
    /// # Errors
    /// Fails when the index cannot be written.
    pub fn delete(&self, id: CollectionId) -> Result<(), AppError> {
        self.store.delete_collection(id)?;
        // Collections can span libraries; each library file keeps its part.
        self.sync.all_libraries_changed();
        Ok(())
    }

    /// Appends models that are not members yet.
    /// # Errors
    /// Fails for a smart collection, unknown models or when the index cannot be written.
    pub fn add(&self, id: CollectionId, models: &[ModelId]) -> Result<(), AppError> {
        self.hand_made(id)?;
        self.store.add_members(id, models)?;
        // Collections can span libraries; each library file keeps its part.
        self.sync.all_libraries_changed();
        Ok(())
    }

    /// # Errors
    /// Fails for a smart collection or when the index cannot be written.
    pub fn remove(&self, id: CollectionId, models: &[ModelId]) -> Result<(), AppError> {
        self.hand_made(id)?;
        self.store.remove_members(id, models)?;
        // Collections can span libraries; each library file keeps its part.
        self.sync.all_libraries_changed();
        Ok(())
    }

    /// Moves one member to position `index` of the shown members (0 = first).
    /// # Errors
    /// Fails for a smart collection, a model that is not a member, or when the index cannot be written.
    pub fn move_to(&self, id: CollectionId, model: ModelId, index: usize) -> Result<(), AppError> {
        self.hand_made(id)?;
        self.store.move_member(id, model, index)?;
        // Collections can span libraries; each library file keeps its part.
        self.sync.all_libraries_changed();
        Ok(())
    }

    /// The collection's models: in the user's order, or as the saved search finds them now.
    /// # Errors
    /// Fails for an unknown collection or when the index cannot be read.
    pub fn models(
        &self,
        id: CollectionId,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<ModelSummary>, usize), AppError> {
        let collection =
            self.store.collection(id)?.ok_or_else(|| AppError::InvalidInput("unknown collection".to_owned()))?;
        if let Some(smart) = collection.smart {
            let request = SearchRequest { text: smart.text, library: smart.library, scope: smart.scope, offset, limit };
            let result = self.search.execute(&request)?;
            return Ok((result.models, result.total));
        }
        let members = self.store.collection_members(id)?;
        let page: Vec<ModelId> = members.iter().skip(offset).take(limit).copied().collect();
        Ok((self.models.summaries(&page)?, members.len()))
    }

    fn hand_made(&self, id: CollectionId) -> Result<(), AppError> {
        let collection =
            self.store.collection(id)?.ok_or_else(|| AppError::InvalidInput("unknown collection".to_owned()))?;
        if collection.smart.is_some() {
            return Err(Refusal::SmartCollection.into());
        }
        Ok(())
    }
}
