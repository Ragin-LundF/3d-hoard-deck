//! 3MF reading: build items with their transforms, grouped per Bambu/Orca plate, each resting on its bed.

mod model;
mod plates;
mod transform;
mod xml;

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use model::{Model, parse_model};
use transform::Transform;
use zip::ZipArchive;

const ROOT_MODEL: &str = "3D/3dmodel.model";
const PLATE_CONFIG: &str = "Metadata/model_settings.config";
/// Keeps memory bounded for the preview; bigger files need a simplified mesh.
const MAX_TRIANGLES: usize = 6_000_000;
/// Unpacked size limit per part, so a small zip cannot expand into gigabytes of XML.
const MAX_PART_BYTES: u64 = 768 * 1024 * 1024;
/// Components may nest; a cycle in a broken file must not recurse forever.
const MAX_DEPTH: usize = 16;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct MeshError(pub String);

/// One build plate: triangles as `x, y, z` floats (Z up, millimetres), centered on the plate, resting on z = 0.
#[derive(Debug, Clone, PartialEq)]
pub struct Plate {
    pub number: u32,
    pub positions: Vec<f32>,
}

/// Reads a 3MF file (read-only). Files without plate information become a single plate 1.
///
/// # Errors
/// Fails for unreadable or broken archives, and for files too large for the preview.
pub fn read_plates(file: &Path) -> Result<Vec<Plate>, MeshError> {
    let archive_error = |error: zip::result::ZipError| MeshError(format!("3MF archive: {error}"));
    let mut archive =
        ZipArchive::new(File::open(file).map_err(|e| MeshError(e.to_string()))?).map_err(archive_error)?;
    let root = root_model_path(&archive).ok_or_else(|| MeshError("3MF without a 3D model part".to_owned()))?;
    let parts = load_parts(&mut archive, &root)?;
    let assignment =
        read_text(&mut archive, PLATE_CONFIG).map(|config| plates::plate_of_objects(&config)).unwrap_or_default();

    let mut plates: BTreeMap<u32, Vec<f32>> = BTreeMap::new();
    let mut triangles = 0;
    for item in &parts[&root].build {
        let mut positions = Vec::new();
        resolve(&parts, &root, item.object, &item.transform, &mut positions, 0)?;
        triangles += positions.len() / 9;
        if triangles > MAX_TRIANGLES {
            return Err(MeshError("this 3MF has too many triangles for the preview yet".to_owned()));
        }
        drop_to_bed(&mut positions);
        plates.entry(assignment.get(&item.object).copied().unwrap_or(1)).or_default().extend(positions);
    }
    Ok(plates
        .into_iter()
        .map(|(number, mut positions)| {
            center_on_plate(&mut positions);
            Plate { number, positions }
        })
        .collect())
}

fn root_model_path<R: Read + std::io::Seek>(archive: &ZipArchive<R>) -> Option<String> {
    let names: Vec<String> = archive.file_names().filter_map(Result::ok).map(std::borrow::Cow::into_owned).collect();
    names
        .iter()
        .find(|name| name.eq_ignore_ascii_case(ROOT_MODEL))
        .or_else(|| names.iter().find(|name| name.starts_with("3D/") && name.to_ascii_lowercase().ends_with(".model")))
        .cloned()
}

/// The root part plus every part its components reference (production extension), parsed once each.
fn load_parts<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    root: &str,
) -> Result<HashMap<String, Model>, MeshError> {
    let mut parts = HashMap::new();
    let mut queue = vec![root.to_owned()];
    let mut budget = MAX_TRIANGLES;
    while let Some(path) = queue.pop() {
        if parts.contains_key(&path) {
            continue;
        }
        let xml = read_text(archive, &path).ok_or_else(|| MeshError(format!("3MF part {path} is missing")))?;
        let model = parse_model(&xml, budget)?;
        budget -= model.objects.values().map(|object| object.mesh.triangles.len()).sum::<usize>();
        queue.extend(model.objects.values().flat_map(|object| object.components.iter().filter_map(|c| c.path.clone())));
        parts.insert(path, model);
    }
    Ok(parts)
}

fn read_text<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>, path: &str) -> Option<String> {
    let mut text = String::new();
    archive.by_name(path).ok()?.take(MAX_PART_BYTES).read_to_string(&mut text).ok()?;
    Some(text)
}

fn resolve(
    parts: &HashMap<String, Model>,
    part: &str,
    object: u32,
    transform: &Transform,
    out: &mut Vec<f32>,
    depth: usize,
) -> Result<(), MeshError> {
    if depth > MAX_DEPTH {
        return Err(MeshError("3MF components nest too deep".to_owned()));
    }
    let Some(definition) = parts.get(part).and_then(|model| model.objects.get(&object)) else { return Ok(()) };
    let mesh = &definition.mesh;
    for triangle in &mesh.triangles {
        // A broken index drops its triangle; skipping single vertices would shift every triangle after it.
        let Some(corners) = triangle.iter().map(|&index| mesh.vertices.get(index).copied()).collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        for vertex in corners {
            out.extend_from_slice(&transform.apply(vertex));
        }
    }
    for component in &definition.components {
        let child_part = component.path.as_deref().unwrap_or(part);
        resolve(parts, child_part, component.object, &component.transform.then(transform), out, depth + 1)?;
    }
    Ok(())
}

/// Like a slicer's "drop to bed": the lowest point of the item touches z = 0.
fn drop_to_bed(positions: &mut [f32]) {
    let lowest = positions.iter().skip(2).step_by(3).copied().fold(f32::INFINITY, f32::min);
    if lowest.is_finite() {
        positions.iter_mut().skip(2).step_by(3).for_each(|z| *z -= lowest);
    }
}

/// Moves a plate's objects so their footprint is centered on the plate origin, keeping their arrangement.
fn center_on_plate(positions: &mut [f32]) {
    for axis in 0..2 {
        let values = positions.iter().skip(axis).step_by(3);
        let (low, high) = values.fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), &v| (low.min(v), high.max(v)));
        if low.is_finite() {
            let middle = low.midpoint(high);
            positions.iter_mut().skip(axis).step_by(3).for_each(|value| *value -= middle);
        }
    }
}

#[cfg(test)]
mod tests;
