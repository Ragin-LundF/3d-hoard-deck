//! A lighter stand-in for STL files too large for the viewer: vertex clustering on a grid, streamed in passes, so
//! even files of several GB never sit in memory whole.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::MeshError;

/// Some exporters append a few bytes after the last triangle.
const BINARY_SLACK: u64 = 512;
/// Grids tried together in one pass, relative to the estimate below; the finest that fits the target wins.
const GRID_FACTORS: [f64; 5] = [0.5, 0.707, 1.0, 1.414, 2.0];

type Triangle = [[f32; 3]; 3];
/// Merged vertex positions, and triangles as indices into them.
type Clustered = (Vec<[f32; 3]>, Vec<[u32; 3]>);

pub struct Simplified {
    /// A binary STL.
    pub stl: Vec<u8>,
    pub original_triangles: u64,
    pub triangles: u64,
}

/// Reduces an STL (binary or ASCII) to about `target` triangles.
/// # Errors
/// Fails when the file cannot be read or holds no triangles.
#[allow(clippy::cast_precision_loss)] // the triangle target only steers an estimate
pub fn simplify_stl(path: &Path, target: usize) -> Result<Simplified, MeshError> {
    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
    let original_triangles = for_each_triangle(path, |triangle| {
        for vertex in triangle {
            for axis in 0..3 {
                low[axis] = low[axis].min(vertex[axis]);
                high[axis] = high[axis].max(vertex[axis]);
            }
        }
    })?;
    let extent = (0..3).map(|axis| high[axis] - low[axis]).fold(0.0_f32, f32::max);
    if original_triangles == 0 || !extent.is_finite() || extent <= 0.0 {
        return Err(MeshError("the STL holds no triangles".to_owned()));
    }
    // Two passes in all: the bounds, then every grid at once.
    // A flat surface of n × n cells gives about 2·n² triangles; rounder or more detailed parts give more.
    let estimate = (target as f64 / 2.0).sqrt();
    let cells: Vec<f64> = GRID_FACTORS.iter().map(|factor| f64::from(extent) / (estimate * factor).max(8.0)).collect();
    let limit = target + target / 5;
    let mut results = cluster(path, low, &cells, limit)?;
    let fits = results
        .iter()
        .rposition(|result| result.as_ref().is_some_and(|(_, triangles)| triangles.len() <= limit))
        .unwrap_or(0);
    let result = results.swap_remove(fits).ok_or_else(|| MeshError("no grid fits".to_owned()))?;
    let (positions, triangles) = result;
    Ok(Simplified { stl: encode(&positions, &triangles), original_triangles, triangles: triangles.len() as u64 })
}

/// One grid's state while the triangles stream by.
#[derive(Default)]
struct Grid {
    /// More triangles than this make the grid useless; it stops growing (and using memory) then.
    limit: Option<usize>,
    full: bool,
    index: HashMap<u64, u32>,
    sums: Vec<([f64; 3], u32)>,
    seen: HashSet<[u32; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl Grid {
    fn add(&mut self, triangle: Triangle, low: [f32; 3], cell: f64) {
        if self.full {
            return;
        }
        let ids = triangle.map(|vertex| {
            let key = cell_key(vertex, low, cell);
            let sums = &mut self.sums;
            let id = *self.index.entry(key).or_insert_with(|| {
                sums.push(([0.0; 3], 0));
                u32::try_from(sums.len() - 1).unwrap_or(u32::MAX)
            });
            if let Some((sum, count)) = self.sums.get_mut(id as usize) {
                for axis in 0..3 {
                    sum[axis] += f64::from(vertex[axis]);
                }
                *count += 1;
            }
            id
        });
        if ids[0] == ids[1] || ids[1] == ids[2] || ids[0] == ids[2] {
            return;
        }
        let mut sorted = ids;
        sorted.sort_unstable();
        if self.seen.insert(sorted) {
            self.triangles.push(ids);
            if self.limit.is_some_and(|limit| self.triangles.len() > limit) {
                *self = Self { full: true, ..Self::default() };
            }
        }
    }

    fn finish(self) -> Clustered {
        #[allow(clippy::cast_possible_truncation)] // averages of f32 coordinates fit an f32
        let positions =
            self.sums.iter().map(|(sum, count)| sum.map(|value| (value / f64::from((*count).max(1))) as f32)).collect();
        (positions, self.triangles)
    }
}

/// Merges all vertices in one grid cell into their average, for every cell size at once; triangles that collapse or
/// repeat are dropped.
/// Only the first (coarsest) grid is never cut off, so there is always a result.
fn cluster(path: &Path, low: [f32; 3], cells: &[f64], limit: usize) -> Result<Vec<Option<Clustered>>, MeshError> {
    let mut grids: Vec<Grid> =
        (0..cells.len()).map(|index| Grid { limit: (index > 0).then_some(limit), ..Grid::default() }).collect();
    for_each_triangle(path, |triangle| {
        for (grid, &cell) in grids.iter_mut().zip(cells) {
            grid.add(triangle, low, cell);
        }
    })?;
    Ok(grids.into_iter().map(|grid| (!grid.full).then(|| grid.finish())).collect())
}

fn cell_key(vertex: [f32; 3], low: [f32; 3], cell: f64) -> u64 {
    let mut key = 0_u64;
    for axis in 0..3 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 21 bits
        let index = ((f64::from(vertex[axis] - low[axis]) / cell).floor().clamp(0.0, 2_097_151.0)) as u64;
        key |= index << (21 * axis);
    }
    key
}

/// Calls `visit` for every triangle and returns how many there were.
fn for_each_triangle(path: &Path, mut visit: impl FnMut(Triangle)) -> Result<u64, MeshError> {
    let error = |error: std::io::Error| MeshError(format!("{}: {error}", path.display()));
    let size = std::fs::metadata(path).map_err(error)?.len();
    let mut reader = BufReader::with_capacity(1 << 20, File::open(path).map_err(error)?);
    let mut header = [0_u8; 84];
    let binary = reader.read_exact(&mut header).is_ok() && {
        let count = u64::from(u32::from_le_bytes([header[80], header[81], header[82], header[83]]));
        let expected = 84 + 50 * count;
        count > 0 && size >= expected && size - expected <= BINARY_SLACK
    };
    if binary {
        let count = u32::from_le_bytes([header[80], header[81], header[82], header[83]]);
        let mut record = [0_u8; 50];
        for _ in 0..count {
            reader.read_exact(&mut record).map_err(error)?;
            let float = |at: usize| f32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]);
            visit([0, 1, 2].map(|corner| [0, 1, 2].map(|axis| float(12 + corner * 12 + axis * 4))));
        }
        return Ok(u64::from(count));
    }
    let mut reader = BufReader::with_capacity(1 << 20, File::open(path).map_err(error)?);
    let (mut corners, mut filled, mut count, mut line) = ([[0.0_f32; 3]; 3], 0, 0_u64, String::new());
    while reader.read_line(&mut line).map_err(error)? > 0 {
        if let Some(rest) = line.trim_start().strip_prefix("vertex") {
            let values: Vec<f32> = rest.split_whitespace().filter_map(|value| value.parse().ok()).collect();
            if let [x, y, z] = values[..] {
                corners[filled] = [x, y, z];
                filled += 1;
                if filled == 3 {
                    visit(corners);
                    (filled, count) = (0, count + 1);
                }
            }
        }
        line.clear();
    }
    Ok(count)
}

fn encode(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<u8> {
    let mut stl = Vec::with_capacity(84 + triangles.len() * 50);
    stl.extend_from_slice(&[0_u8; 80]);
    stl.extend_from_slice(&u32::try_from(triangles.len()).unwrap_or(u32::MAX).to_le_bytes());
    for triangle in triangles {
        // A zero normal: the viewer computes its own.
        stl.extend_from_slice(&[0_u8; 12]);
        for &id in triangle {
            for value in positions.get(id as usize).copied().unwrap_or_default() {
                stl.extend_from_slice(&value.to_le_bytes());
            }
        }
        stl.extend_from_slice(&[0_u8; 2]);
    }
    stl
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// A wavy height field of `n × n` quads: 2·n² triangles, 100 mm wide.
    #[allow(clippy::cast_precision_loss)]
    fn fine_surface(n: usize) -> Vec<u8> {
        let height = |x: f32, y: f32| (x * 0.2).sin() * (y * 0.2).cos() * 5.0;
        let point = |i: usize, j: usize| {
            let (x, y) = (i as f32 * 100.0 / n as f32, j as f32 * 100.0 / n as f32);
            [x, y, height(x, y)]
        };
        let mut triangles = Vec::new();
        for i in 0..n {
            for j in 0..n {
                triangles.push([point(i, j), point(i + 1, j), point(i + 1, j + 1)]);
                triangles.push([point(i, j), point(i + 1, j + 1), point(i, j + 1)]);
            }
        }
        let mut stl = vec![0_u8; 80];
        stl.extend_from_slice(&u32::try_from(triangles.len()).expect("fits").to_le_bytes());
        for triangle in triangles {
            stl.extend_from_slice(&[0_u8; 12]);
            for vertex in triangle {
                for value in vertex {
                    stl.extend_from_slice(&value.to_le_bytes());
                }
            }
            stl.extend_from_slice(&[0_u8; 2]);
        }
        stl
    }

    #[test]
    fn a_fine_mesh_shrinks_to_about_the_target_and_keeps_its_size() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("fine.stl");
        std::fs::write(&path, fine_surface(400)).expect("write");
        let simplified = simplify_stl(&path, 20_000).expect("simplify");
        assert_eq!(simplified.original_triangles, 320_000);
        assert!((10_000..=24_000).contains(&simplified.triangles), "{} triangles", simplified.triangles);
        assert_eq!(simplified.stl.len() as u64, 84 + 50 * simplified.triangles);
        let xs: Vec<f32> = simplified.stl[84..]
            .chunks(50)
            .flat_map(|r| [24, 36, 48].map(|at| f32::from_le_bytes([r[at - 12], r[at - 11], r[at - 10], r[at - 9]])))
            .collect();
        let (min, max) = xs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(min < 1.0 && max > 99.0, "the outline is kept: {min}..{max}");
    }

    #[test]
    fn ascii_files_are_read_too() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("tri.stl");
        let mut file = File::create(&path).expect("create");
        write!(file, "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 10 0 0\nvertex 0 10 0\nendloop\nendfacet\nendsolid t\n").expect("write");
        let simplified = simplify_stl(&path, 100).expect("simplify");
        assert_eq!((simplified.original_triangles, simplified.triangles), (1, 1));
    }
}
