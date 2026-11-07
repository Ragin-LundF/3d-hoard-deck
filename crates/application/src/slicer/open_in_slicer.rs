use std::path::Path;
use std::sync::Arc;

use hoarddeck_domain::{FileFormat, FileKind, ModelFile, ModelId, RelPath};

use super::ManageSlicers;
use crate::AppError;
use crate::ports::{AppLauncher, LibraryRepository, ModelRepository};

/// More files than this are refused: a whole pack opened by accident would flood the slicer.
const MAX_FILES: usize = 50;

/// Opens a model's files in a slicer: the chosen files, else the model's 3MF projects, else its main mesh files,
/// else its slicer projects (e.g. Lychee).
pub struct OpenInSlicer {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub slicers: Arc<ManageSlicers>,
    pub launcher: Arc<dyn AppLauncher>,
}

impl OpenInSlicer {
    /// `slicer`: index in the list, or `None` for the first one that opens all files. Returns the slicer's name.
    /// # Errors
    /// Fails for an unknown model, files outside the model, no matching slicer, or when the slicer cannot start.
    pub fn execute(&self, model: ModelId, files: &[RelPath], slicer: Option<usize>) -> Result<String, AppError> {
        let detail = self.models.get(model)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))?;
        let library =
            self.libraries.get(detail.summary.library)?.ok_or(AppError::LibraryNotFound(detail.summary.library))?;
        let chosen: Vec<&ModelFile> = if files.is_empty() {
            default_files(&detail.files)
        } else {
            files
                .iter()
                .map(|path| detail.files.iter().find(|file| &file.path == path))
                .collect::<Option<_>>()
                .ok_or_else(|| AppError::InvalidInput("a file does not belong to this model".to_owned()))?
        };
        if chosen.is_empty() {
            return Err(AppError::NoSlicer);
        }
        if chosen.len() > MAX_FILES {
            return Err(AppError::InvalidInput(format!(
                "{} files are too many to open at once; open single files from the file list",
                chosen.len()
            )));
        }
        let extensions: Vec<String> = chosen.iter().map(|file| real_extension(file)).collect();
        let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
        let list = self.slicers.list()?;
        let slicer = match slicer {
            Some(index) => {
                list.slicers.get(index).filter(|slicer| extensions.iter().all(|extension| slicer.opens(extension)))
            }
            None => list.for_extensions(&extensions),
        }
        .ok_or(AppError::NoSlicer)?;
        let paths: Vec<String> = chosen
            .iter()
            .map(|file| {
                let mut path = Path::new(&library.root).to_path_buf();
                path.extend(file.path.segments());
                path.display().to_string()
            })
            .collect();
        self.launcher.launch(slicer, &paths).map_err(AppError::LaunchFailed)?;
        Ok(slicer.name.clone())
    }
}

/// The type a slicer must accept: the sniffed mesh format (`dragon.stl.txt` is an STL), else the name's extension.
fn real_extension(file: &ModelFile) -> String {
    match file.kind {
        FileKind::Mesh(format) => format.code().to_owned(),
        _ => file.path.name().rsplit_once('.').map_or(String::new(), |(_, extension)| extension.to_ascii_lowercase()),
    }
}

/// 3MF projects keep plates and settings, so they win; otherwise the meshes of the main folder, or of the first
/// variant (e.g. `Supported`) when the main folder has none.
fn default_files(files: &[ModelFile]) -> Vec<&ModelFile> {
    let projects: Vec<&ModelFile> =
        files.iter().filter(|file| file.kind == FileKind::Mesh(FileFormat::ThreeMf)).collect();
    if !projects.is_empty() {
        return projects;
    }
    let mut meshes: Vec<&ModelFile> = files.iter().filter(|file| matches!(file.kind, FileKind::Mesh(_))).collect();
    if meshes.is_empty() {
        meshes = files.iter().filter(|file| file.kind == FileKind::SlicerProject).collect();
    }
    let first_variant = meshes.iter().map(|file| file.variant.clone()).min();
    meshes.into_iter().filter(|file| Some(file.variant.clone()) == first_variant).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, kind: FileKind, variant: Option<&str>) -> ModelFile {
        ModelFile { path: RelPath::new(path).expect("valid"), kind, size: 1, variant: variant.map(str::to_owned) }
    }

    const STL: FileKind = FileKind::Mesh(FileFormat::Stl);

    #[test]
    fn projects_win_then_main_meshes_then_the_first_variant() {
        let with_project = [file("m/a.stl", STL, None), file("m/plate.3mf", FileKind::Mesh(FileFormat::ThreeMf), None)];
        assert_eq!(default_files(&with_project).iter().map(|f| f.path.name()).collect::<Vec<_>>(), vec!["plate.3mf"]);
        let main_and_variant = [
            file("m/a.stl", STL, None),
            file("m/Supported/a.stl", STL, Some("Supported")),
            file("m/c.jpg", FileKind::Image, None),
        ];
        assert_eq!(default_files(&main_and_variant).len(), 1);
        let variants_only =
            [file("m/Unsupported/a.stl", STL, Some("Unsupported")), file("m/Supported/a.stl", STL, Some("Supported"))];
        assert_eq!(default_files(&variants_only)[0].variant.as_deref(), Some("Supported"));
    }
}
