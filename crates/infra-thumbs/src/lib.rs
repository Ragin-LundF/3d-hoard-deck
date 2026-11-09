//! Thumbnail cache for cover images.

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader, Rgb, RgbImage};

/// Longest edge of a cover thumbnail; sharp on a 2× display at the largest card size.
const EDGE: u32 = 512;
const QUALITY: u8 = 82;
/// Transparent areas become this light neutral gray instead of JPEG's default black.
const BACKGROUND: [u8; 3] = [0xE9, 0xEC, 0xF1];

/// Makes temporary file names unique across threads as well as processes.
static WRITES: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
#[error("thumbnail: {0}")]
pub struct ThumbnailError(String);

pub struct ThumbnailCache {
    dir: PathBuf,
}

impl ThumbnailCache {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The thumbnail if it is cached already; costs one `stat` of the source, no read.
    ///
    /// # Errors
    /// Fails when the source cannot be found.
    pub fn cached(&self, source: &Path) -> Result<Option<PathBuf>, ThumbnailError> {
        let (target, _) = self.target(source)?;
        Ok(target.exists().then_some(target))
    }

    /// Where the thumbnail of this version of `source` lives, and its key.
    fn target(&self, source: &Path) -> Result<(PathBuf, u64), ThumbnailError> {
        let metadata = fs::metadata(source).map_err(|e| ThumbnailError(e.to_string()))?;
        let modified =
            metadata.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        let key = fnv1a(format!("{}|{}|{modified}", source.display(), metadata.len()).as_bytes());
        Ok((self.dir.join(format!("{key:016x}.jpg")), key))
    }

    /// The cached thumbnail of `source`, created on first use. Safe to call from several threads: the file is
    /// written under a temporary name and renamed, so readers never see half a JPEG.
    ///
    /// # Errors
    /// Fails when the source cannot be read or decoded, or the cache folder cannot be written.
    pub fn get_or_create(&self, source: &Path) -> Result<PathBuf, ThumbnailError> {
        let (target, key) = self.target(source)?;
        if target.exists() {
            return Ok(target);
        }
        let image = decode_upright(source)?;
        let thumbnail = if image.width().max(image.height()) > EDGE {
            image.resize(EDGE, EDGE, FilterType::Triangle)
        } else {
            image
        };
        fs::create_dir_all(&self.dir).map_err(|e| ThumbnailError(e.to_string()))?;
        let write = WRITES.fetch_add(1, Ordering::Relaxed);
        let temporary = self.dir.join(format!("{key:016x}.{}.{write}.tmp", std::process::id()));
        let file = fs::File::create(&temporary).map_err(|e| ThumbnailError(e.to_string()))?;
        flatten(&thumbnail)
            .write_with_encoder(JpegEncoder::new_with_quality(BufWriter::new(file), QUALITY))
            .map_err(|e| ThumbnailError(e.to_string()))?;
        fs::rename(&temporary, &target).map_err(|e| ThumbnailError(e.to_string()))?;
        Ok(target)
    }
}

/// Decodes and turns the image the way the camera's EXIF orientation says.
fn decode_upright(source: &Path) -> Result<DynamicImage, ThumbnailError> {
    let error = |e: image::ImageError| ThumbnailError(e.to_string());
    let reader = ImageReader::open(source)
        .map_err(|e| ThumbnailError(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| ThumbnailError(e.to_string()))?;
    let mut decoder = reader.into_decoder().map_err(error)?;
    let orientation = decoder.orientation().map_err(error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(error)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// RGB with transparency blended onto [`BACKGROUND`].
fn flatten(image: &DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |channel: u8, back: u8| {
            let mixed = (u16::from(channel) * u16::from(a) + u16::from(back) * (255 - u16::from(a))) / 255;
            u8::try_from(mixed).unwrap_or(u8::MAX)
        };
        Rgb([blend(r, BACKGROUND[0]), blend(g, BACKGROUND[1]), blend(b, BACKGROUND[2])])
    })
}

/// Stable across Rust versions, unlike `DefaultHasher`, so the cache survives app updates.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3))
}

impl hoarddeck_application::ports::PreviewImages for ThumbnailCache {
    fn preview_jpeg(&self, file: &Path) -> Result<Vec<u8>, String> {
        let thumbnail = self.get_or_create(file).map_err(|error| error.to_string())?;
        fs::read(thumbnail).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_images_are_scaled_once_and_reused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("cover.png");
        image::RgbImage::from_pixel(2000, 1000, image::Rgb([255, 122, 69])).save(&source).expect("write png");
        let cache = ThumbnailCache::new(dir.path().join("thumbnails"));

        let first = cache.get_or_create(&source).expect("create");
        let thumbnail = image::open(&first).expect("valid jpeg");
        assert_eq!((thumbnail.width(), thumbnail.height()), (512, 256));
        assert_eq!(cache.get_or_create(&source).expect("cached"), first, "same file, same thumbnail");

        image::RgbImage::from_pixel(1000, 1000, image::Rgb([0, 0, 0])).save(&source).expect("rewrite png");
        assert_ne!(cache.get_or_create(&source).expect("recreate"), first, "a changed image gets a new thumbnail");
    }

    #[test]
    fn transparent_areas_become_light_instead_of_black() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("logo.png");
        image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0])).save(&source).expect("write png");
        let thumbnail = ThumbnailCache::new(dir.path().join("thumbnails")).get_or_create(&source).expect("create");
        let pixel = image::open(thumbnail).expect("jpeg").to_rgb8().get_pixel(32, 32).0;
        assert!(pixel.iter().all(|&channel| channel > 200), "got {pixel:?}");
    }

    #[test]
    fn broken_images_fail_instead_of_caching_garbage() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("broken.jpg");
        fs::write(&source, b"not an image").expect("write");
        assert!(ThumbnailCache::new(dir.path().join("thumbnails")).get_or_create(&source).is_err());
    }
}
