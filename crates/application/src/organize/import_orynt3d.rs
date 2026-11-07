use std::sync::Arc;

use hoarddeck_domain::{LibraryId, ModelKind, ModelMeta, TagName};

use super::UpdateModelMeta;
use crate::AppError;
use crate::ports::{ImportedMeta, LibraryRepository, MetaStore, ModelRepository, Orynt3dReader};

/// Takes names, notes and tags from `Orynt3D`'s files into 3D Hoard Deck. Only reads those files; running it again
/// adds nothing twice and never overwrites what the user set here.
pub struct ImportOrynt3d {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub reader: Arc<dyn Orynt3dReader>,
    pub update: Arc<UpdateModelMeta>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub updated: usize,
    pub unreadable: usize,
}

impl ImportOrynt3d {
    /// # Errors
    /// Fails for an unknown library or when the index cannot be written. Broken files are counted, not fatal.
    pub fn execute(&self, id: LibraryId) -> Result<ImportReport, AppError> {
        let library = self.libraries.get(id)?.ok_or(AppError::LibraryNotFound(id))?;
        let mut report = ImportReport::default();
        for document in self.models.search_documents(id)?.into_iter().filter(|doc| doc.kind == ModelKind::Folder) {
            let imported = match self.reader.read(&library.root, &document.path) {
                Ok(Some(imported)) => imported,
                Ok(None) => continue,
                Err(_) => {
                    report.unreadable += 1;
                    continue;
                }
            };
            let current = self.meta.meta(id, &document.path)?;
            let merged = merge(current.clone(), imported);
            if merged != current {
                self.update.execute(document.id, merged)?;
                report.updated += 1;
            }
        }
        Ok(report)
    }
}

/// Adds what is missing: the name and notes only when none are set here, tags as a union.
fn merge(mut meta: ModelMeta, imported: ImportedMeta) -> ModelMeta {
    if meta.display_name.is_none() {
        meta.display_name = imported.name;
    }
    if meta.notes.trim().is_empty() {
        meta.notes = imported.notes;
    }
    meta.tags.extend(imported.tags.iter().filter_map(|tag| TagName::new(tag).ok()));
    meta.normalized().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merging_fills_gaps_and_never_overwrites() {
        let mine = ModelMeta {
            display_name: Some("My Dragon".to_owned()),
            tags: [TagName::new("Resin").expect("valid")].into_iter().collect(),
            ..ModelMeta::default()
        };
        let imported = ImportedMeta {
            name: Some("Dragon v2".to_owned()),
            notes: "0.05 mm".to_owned(),
            tags: vec!["resin".to_owned(), "Fantasy".to_owned()],
        };
        let merged = merge(mine, imported.clone());
        assert_eq!(merged.display_name.as_deref(), Some("My Dragon"));
        assert_eq!(merged.notes, "0.05 mm");
        assert_eq!(merged.tags.iter().map(TagName::as_str).collect::<Vec<_>>(), vec!["Fantasy", "Resin"]);
        assert_eq!(merge(merged.clone(), imported), merged, "a second import changes nothing");
    }
}
