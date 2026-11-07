use std::sync::Arc;

use hoarddeck_domain::{Collection, CollectionId, ModelId, collection_name};

use crate::ports::{CollectionStore, ModelRepository};
use crate::search::{SearchModels, SearchRequest};
use crate::{AppError, ModelSummary};

/// Hand-made collections keep the order the user gives them; smart ones are a saved search.
pub struct ManageCollections {
    pub store: Arc<dyn CollectionStore>,
    pub models: Arc<dyn ModelRepository>,
    pub search: Arc<SearchModels>,
}

impl ManageCollections {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn list(&self) -> Result<Vec<(Collection, Option<u32>)>, AppError> {
        Ok(self.store.list_collections()?)
    }

    /// `query`: search text for a smart collection, `None` for a hand-made one.
    /// # Errors
    /// Fails for an invalid name or when the index cannot be written.
    pub fn create(&self, name: &str, query: Option<&str>) -> Result<Collection, AppError> {
        let query = query.map(str::trim).filter(|query| !query.is_empty());
        Ok(self.store.create_collection(&collection_name(name)?, query)?)
    }

    /// # Errors
    /// Fails for an invalid name or when the index cannot be written.
    pub fn rename(&self, id: CollectionId, name: &str) -> Result<(), AppError> {
        Ok(self.store.rename_collection(id, &collection_name(name)?)?)
    }

    /// Deletes the collection only; its models stay in the library.
    /// # Errors
    /// Fails when the index cannot be written.
    pub fn delete(&self, id: CollectionId) -> Result<(), AppError> {
        Ok(self.store.delete_collection(id)?)
    }

    /// Appends models that are not members yet.
    /// # Errors
    /// Fails for a smart collection, unknown models or when the index cannot be written.
    pub fn add(&self, id: CollectionId, models: &[ModelId]) -> Result<(), AppError> {
        let mut members = self.hand_made_members(id)?;
        for model in models {
            if !members.contains(model) {
                members.push(*model);
            }
        }
        Ok(self.store.set_collection_members(id, &members)?)
    }

    /// # Errors
    /// Fails for a smart collection or when the index cannot be written.
    pub fn remove(&self, id: CollectionId, models: &[ModelId]) -> Result<(), AppError> {
        let mut members = self.hand_made_members(id)?;
        members.retain(|member| !models.contains(member));
        Ok(self.store.set_collection_members(id, &members)?)
    }

    /// Moves one member to position `index` (0 = first); later positions shift.
    /// # Errors
    /// Fails for a smart collection, a model that is not a member, or when the index cannot be written.
    pub fn move_to(&self, id: CollectionId, model: ModelId, index: usize) -> Result<(), AppError> {
        let mut members = self.hand_made_members(id)?;
        let from = members
            .iter()
            .position(|member| *member == model)
            .ok_or_else(|| AppError::InvalidInput("the model is not in this collection".to_owned()))?;
        let moved = members.remove(from);
        members.insert(index.min(members.len()), moved);
        Ok(self.store.set_collection_members(id, &members)?)
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
        if let Some(query) = collection.query {
            let result =
                self.search.execute(&SearchRequest { text: query, library: None, scope: None, offset, limit })?;
            return Ok((result.models, result.total));
        }
        let members = self.store.collection_members(id)?;
        let page: Vec<ModelId> = members.iter().skip(offset).take(limit).copied().collect();
        Ok((self.models.summaries(&page)?, members.len()))
    }

    fn hand_made_members(&self, id: CollectionId) -> Result<Vec<ModelId>, AppError> {
        let collection =
            self.store.collection(id)?.ok_or_else(|| AppError::InvalidInput("unknown collection".to_owned()))?;
        if collection.query.is_some() {
            return Err(AppError::InvalidInput("a smart collection fills itself from its search".to_owned()));
        }
        Ok(self.store.collection_members(id)?)
    }
}
