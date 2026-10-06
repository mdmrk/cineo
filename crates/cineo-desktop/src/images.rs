//! Image bytes for egui, fetched through `cineo-net` (ADR-0011,
//! docs/SECURITY.md §Desktop UI). Only `http(s)` URIs are handled; the
//! `egui_extras` image loader decodes what this returns.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::task::Poll;

use cineo_net::AddonClient;
use eframe::egui::{
    self,
    load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError},
    mutex::Mutex,
};
use url::Url;

/// Largest image download accepted.
pub(crate) const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
/// Largest width or height accepted, checked from the header before
/// decoding (decompression bombs).
pub(crate) const MAX_IMAGE_SIDE: u32 = 4096;

type Entry = Poll<Result<Arc<[u8]>, String>>;

/// A [`BytesLoader`] for `http(s)` images under the network policy.
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

impl BytesLoader for NetImageLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        let Some(url) = Url::parse(uri)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https"))
        else {
            return Err(LoadError::NotSupported);
        };
        let mut cache = self.cache.lock();
        match cache.get(uri) {
            Some(Poll::Ready(Ok(bytes))) => {
                return Ok(BytesPoll::Ready {
                    size: None,
                    bytes: Bytes::Shared(Arc::clone(bytes)),
                    mime: None,
                });
            }
            Some(Poll::Ready(Err(err))) => return Err(LoadError::Loading(err.clone())),
            Some(Poll::Pending) => return Ok(BytesPoll::Pending { size: None }),
            None => {}
        }
        cache.insert(uri.to_owned(), Poll::Pending);
        drop(cache);

        let client = Arc::clone(&self.client);
        let cache = Arc::clone(&self.cache);
        let ctx = ctx.clone();
        let uri = uri.to_owned();
        self.runtime.spawn(async move {
            let result = client
                .fetch_image(&url, MAX_IMAGE_BYTES)
                .await
                .map_err(|err| err.to_string())
                .and_then(|bytes| {
                    check_dimensions(&bytes)?;
                    Ok(Arc::<[u8]>::from(bytes))
                });
            if let Err(err) = &result {
                tracing::debug!(origin = %url.origin().ascii_serialization(), %err, "image not loaded");
            }
            cache.lock().insert(uri, Poll::Ready(result));
            ctx.request_repaint();
        });
        Ok(BytesPoll::Pending { size: None })
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
                Poll::Ready(Ok(bytes)) => bytes.len(),
                _ => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.cache.lock().values().any(Poll::is_pending)
    }
}

/// Accepts only jpeg, png or webp images whose header declares at most
/// [`MAX_IMAGE_SIDE`] pixels per side.
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
    fn non_images_and_disabled_formats_are_rejected() {
        assert!(check_dimensions(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_err());
        // GIF magic: the format exists, but its decoder is not compiled in.
        assert!(check_dimensions(b"GIF89a\x01\x00\x01\x00\x00\x00\x00;").is_err());
    }
}
