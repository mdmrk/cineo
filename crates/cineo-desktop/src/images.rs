//! Images for egui, fetched through `cineo-net` (ADR-0011,
//! docs/SECURITY.md §Desktop UI) and decoded off the UI thread. Only
//! `http(s)` URIs are handled.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::task::Poll;

use cineo_net::AddonClient;
use eframe::egui::{
    self, ColorImage,
    load::{ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint},
    mutex::Mutex,
};
use image::imageops::FilterType;
use url::Url;

pub(crate) const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_IMAGE_SIDE: u32 = 4096;
const SIZE_HEADROOM: f32 = 2.0;

type Entry = Poll<Result<Arc<ColorImage>, String>>;

pub(crate) struct NetImageLoader {
    client: Arc<AddonClient>,
    runtime: tokio::runtime::Handle,
    cache: Arc<Mutex<HashMap<String, Entry>>>,
}

impl NetImageLoader {
    const ID: &'static str = egui::generate_loader_id!(NetImageLoader);

    pub(crate) fn new(client: Arc<AddonClient>, runtime: tokio::runtime::Handle) -> Self {
        Self {
            client,
            runtime,
            cache: Arc::default(),
        }
    }
}

impl ImageLoader for NetImageLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, size_hint: SizeHint) -> ImageLoadResult {
        let source = egui::decode_animated_image_uri(uri).map_or(uri, |(source, _)| source);
        let Some(url) = Url::parse(source)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https"))
        else {
            return Err(LoadError::NotSupported);
        };
        let mut cache = self.cache.lock();
        match cache.get(uri) {
            Some(Poll::Ready(Ok(image))) => {
                return Ok(ImagePoll::Ready {
                    image: Arc::clone(image),
                });
            }
            Some(Poll::Ready(Err(err))) => return Err(LoadError::Loading(err.clone())),
            Some(Poll::Pending) => return Ok(ImagePoll::Pending { size: None }),
            None => {}
        }
        cache.insert(uri.to_owned(), Poll::Pending);
        drop(cache);

        let client = Arc::clone(&self.client);
        let cache = Arc::clone(&self.cache);
        let ctx = ctx.clone();
        let uri = uri.to_owned();
        let runtime = self.runtime.clone();
        self.runtime.spawn(async move {
            let result = match client.fetch_image(&url, MAX_IMAGE_BYTES).await {
                Ok(bytes) => runtime
                    .spawn_blocking(move || decode(&bytes, size_hint))
                    .await
                    .unwrap_or_else(|err| Err(err.to_string())),
                Err(err) => Err(err.to_string()),
            };
            if let Err(err) = &result {
                tracing::debug!(origin = %url.origin().ascii_serialization(), %err, "image not loaded");
            }
            let result = result.map(Arc::new);
            // Released before repainting: egui may hold its own lock while asking this cache.
            cache.lock().insert(uri, Poll::Ready(result));
            ctx.request_repaint();
        });
        Ok(ImagePoll::Pending { size: None })
    }

    fn forget(&self, uri: &str) {
        self.cache.lock().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .values()
            .map(|entry| match entry {
                Poll::Ready(Ok(image)) => image.pixels.len() * size_of::<egui::Color32>(),
                _ => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.cache.lock().values().any(Poll::is_pending)
    }
}

pub(crate) fn decode(bytes: &[u8], hint: SizeHint) -> Result<ColorImage, String> {
    check_dimensions(bytes)?;
    let image =
        image::load_from_memory(bytes).map_err(|err| format!("unsupported image: {err}"))?;
    let (width, height) = decoded_size(image.width(), image.height(), hint);
    let image = if (width, height) == (image.width(), image.height()) {
        image
    } else {
        image.resize_exact(width, height, FilterType::Triangle)
    };
    let rgba = image.into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "pixel sizes up to MAX_IMAGE_SIDE, rounded, at least 1"
)]
pub(crate) fn decoded_size(width: u32, height: u32, hint: SizeHint) -> (u32, u32) {
    let (w, h) = (width as f32, height as f32);
    let scale = match hint {
        SizeHint::Size {
            width: target_w,
            height: target_h,
            ..
        } => (target_w as f32 / w).max(target_h as f32 / h),
        SizeHint::Width(target) => target as f32 / w,
        SizeHint::Height(target) => target as f32 / h,
        SizeHint::Scale(_) => return (width, height),
    } * SIZE_HEADROOM;
    if !(scale > 0.0 && scale < 1.0) {
        return (width, height);
    }
    let side = |original: f32| ((original * scale).round() as u32).max(1);
    (side(w), side(h))
}

pub(crate) fn check_dimensions(bytes: &[u8]) -> Result<(), String> {
    let (width, height) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?
        .into_dimensions()
        .map_err(|err| format!("unsupported image: {err}"))?;
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        return Err(format!("image too large ({width}×{height})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // Test helpers panic on purpose: a panic is a failed assertion.
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::GrayImage::new(width, height)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn images_within_the_pixel_limit_are_accepted() {
        assert_eq!(check_dimensions(&png(300, 450)), Ok(()));
        assert_eq!(check_dimensions(&png(MAX_IMAGE_SIDE, 1)), Ok(()));
    }

    #[test]
    fn oversized_images_are_rejected_before_decoding() {
        let err = check_dimensions(&png(MAX_IMAGE_SIDE + 1, 1)).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn images_are_downscaled_to_cover_twice_the_requested_size() {
        let hint = SizeHint::Size {
            width: 128,
            height: 192,
            maintain_aspect_ratio: true,
        };
        assert_eq!(decoded_size(1000, 1500, hint), (256, 384));
        assert_eq!(decoded_size(3000, 1500, hint), (768, 384));
        let decoded = decode(&png(1000, 1500), hint).unwrap();
        assert_eq!(decoded.size, [256, 384]);
    }

    #[test]
    fn images_are_never_upscaled() {
        let hint = SizeHint::Size {
            width: 1280,
            height: 440,
            maintain_aspect_ratio: true,
        };
        assert_eq!(decoded_size(300, 450, hint), (300, 450));
        assert_eq!(
            decoded_size(300, 450, SizeHint::Scale(1.0.into())),
            (300, 450)
        );
        let empty = SizeHint::Size {
            width: 0,
            height: 0,
            maintain_aspect_ratio: true,
        };
        assert_eq!(decoded_size(300, 450, empty), (300, 450));
    }

    #[test]
    fn non_images_and_disabled_formats_are_rejected() {
        assert!(check_dimensions(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_err());
        assert!(check_dimensions(b"GIF89a\x01\x00\x01\x00\x00\x00\x00;").is_err());
    }
}
