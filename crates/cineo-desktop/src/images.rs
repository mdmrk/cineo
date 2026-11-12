use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::task::Poll;

use cineo_net::{AddonClient, MAX_IMAGE_BYTES};
use eframe::egui::{
    self, ColorImage,
    load::{ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint},
    mutex::Mutex,
};
use fast_image_resize::{self as fir, FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageDecoder};
use tokio::sync::Semaphore;
use url::Url;

use crate::image_disk::ImageDisk;

pub(crate) const MAX_IMAGE_SIDE: u32 = 4096;
const SIZE_HEADROOM: f32 = 2.0;

const TEXTURE_BUDGET: usize = 256 * 1024 * 1024;
const PROTECTED_PASSES: u64 = 2;

type Entry = Poll<Result<Arc<ColorImage>, String>>;
type SharedTextures = Arc<Mutex<Textures>>;

#[derive(Debug, Clone, Copy)]
struct Held {
    bytes: usize,
    painted: Option<u64>,
}

#[derive(Debug, Default)]
struct Textures {
    held: HashMap<String, Held>,
}

impl Textures {
    fn decoded(&mut self, uri: String, [width, height]: [usize; 2]) {
        let bytes = width.saturating_mul(height).saturating_mul(16) / 3;
        self.held.insert(
            uri,
            Held {
                bytes,
                painted: None,
            },
        );
    }

    fn take_over_budget(&mut self, pass: u64, budget: usize) -> Vec<String> {
        let painted = self
            .held
            .iter()
            .filter_map(|(uri, held)| Some((uri.clone(), held.painted?, held.bytes)))
            .collect();
        let evicted = over_budget(painted, pass, budget);
        for uri in &evicted {
            self.held.remove(uri);
        }
        evicted
    }
}

fn over_budget(mut held: Vec<(String, u64, usize)>, pass: u64, budget: usize) -> Vec<String> {
    let mut total: usize = held.iter().map(|&(_, _, bytes)| bytes).sum();
    held.sort_by_key(|&(_, painted, _)| painted);
    let mut evicted = Vec::new();
    for (uri, painted, bytes) in held {
        if total <= budget || painted + PROTECTED_PASSES > pass {
            break;
        }
        total -= bytes;
        evicted.push(uri);
    }
    evicted
}

fn textures_id() -> egui::Id {
    egui::Id::new("cineo-image-textures")
}

pub(crate) fn painted(ctx: &egui::Context, uri: &str) {
    let Some(textures) = ctx.data(|d| d.get_temp::<SharedTextures>(textures_id())) else {
        return;
    };
    let pass = ctx.cumulative_pass_nr();
    if let Some(held) = textures.lock().held.get_mut(uri) {
        held.painted = Some(pass);
    }
}

pub(crate) fn evict(ctx: &egui::Context) {
    evict_over(ctx, ctx.cumulative_pass_nr(), TEXTURE_BUDGET);
}

fn evict_over(ctx: &egui::Context, pass: u64, budget: usize) {
    let Some(textures) = ctx.data(|d| d.get_temp::<SharedTextures>(textures_id())) else {
        return;
    };
    let evicted = textures.lock().take_over_budget(pass, budget);
    for uri in evicted {
        ctx.forget_image(&uri);
    }
}

pub(crate) struct NetImageLoader {
    client: AddonClient,
    runtime: tokio::runtime::Handle,
    cache: Arc<Mutex<HashMap<String, Entry>>>,
    decoders: Arc<Semaphore>,
    disk: ImageDisk,
    textures: SharedTextures,
}

impl NetImageLoader {
    const ID: &'static str = egui::generate_loader_id!(NetImageLoader);

    pub(crate) fn new(
        client: AddonClient,
        runtime: tokio::runtime::Handle,
        disk: ImageDisk,
    ) -> Self {
        let cores = std::thread::available_parallelism().map_or(2, usize::from);
        Self {
            client,
            runtime,
            cache: Arc::default(),
            decoders: Arc::new(Semaphore::new(cores)),
            disk,
            textures: Arc::default(),
        }
    }
}

struct Fetch {
    client: AddonClient,
    runtime: tokio::runtime::Handle,
    decoders: Arc<Semaphore>,
    disk: ImageDisk,
    url: Url,
    hint: SizeHint,
}

impl Fetch {
    async fn image(self) -> Result<ColorImage, String> {
        if let Some(image) = self.cached().await {
            return Ok(image);
        }
        let bytes = self
            .client
            .fetch_image(&self.url, MAX_IMAGE_BYTES)
            .await
            .map_err(|err| err.to_string())?;
        let _slot = self.decoders.acquire().await;
        let (disk, key, hint) = (self.disk, self.url.to_string(), self.hint);
        self.runtime
            .spawn_blocking(move || {
                let image = decode(&bytes, hint)?;
                disk.write(&key, &bytes);
                Ok(image)
            })
            .await
            .unwrap_or_else(|err| Err(err.to_string()))
    }

    async fn cached(&self) -> Option<ColorImage> {
        let _slot = self.decoders.acquire().await;
        let (disk, key, hint) = (self.disk.clone(), self.url.to_string(), self.hint);
        self.runtime
            .spawn_blocking(move || {
                let bytes = disk.read(&key)?;
                let image = decode(&bytes, hint);
                if image.is_err() {
                    disk.remove(&key);
                }
                image.ok()
            })
            .await
            .ok()
            .flatten()
    }
}

pub(crate) fn install(ctx: &egui::Context, loader: NetImageLoader) {
    ctx.options_mut(|o| o.reduce_texture_memory = true);
    let textures = Arc::clone(&loader.textures);
    ctx.data_mut(|d| d.insert_temp(textures_id(), textures));
    ctx.add_image_loader(Arc::new(loader));
}

impl ImageLoader for NetImageLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, size_hint: SizeHint) -> ImageLoadResult {
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
        let source = egui::decode_animated_image_uri(uri).map_or(uri, |(source, _)| source);
        let Some(url) = Url::parse(source)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https"))
        else {
            return Err(LoadError::NotSupported);
        };
        cache.insert(uri.to_owned(), Poll::Pending);
        drop(cache);

        let cache = Arc::clone(&self.cache);
        let textures = Arc::clone(&self.textures);
        let ctx = ctx.clone();
        let uri = uri.to_owned();
        let origin = url.origin().ascii_serialization();
        let fetch = Fetch {
            client: self.client.clone(),
            runtime: self.runtime.clone(),
            decoders: Arc::clone(&self.decoders),
            disk: self.disk.clone(),
            url,
            hint: size_hint,
        };
        self.runtime.spawn(async move {
            let result = fetch.image().await;
            if let Err(err) = &result {
                tracing::debug!(%origin, %err, "image not loaded");
            }
            if let Ok(image) = &result {
                textures.lock().decoded(uri.clone(), image.size);
            }
            let result = result.map(Arc::new);
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
    let image = DynamicImage::from_decoder(checked_decoder(bytes)?)
        .map_err(|err| format!("unsupported image: {err}"))?;
    let (from_width, from_height) = (image.width(), image.height());
    let (width, height) = decoded_size(from_width, from_height, hint);
    let (kind, pixels) = match image {
        DynamicImage::ImageRgb8(rgb) => (PixelType::U8x3, rgb.into_raw()),
        other => (PixelType::U8x4, other.into_rgba8().into_raw()),
    };
    let pixels = if (width, height) == (from_width, from_height) {
        pixels
    } else {
        let from = fir::images::Image::from_vec_u8(from_width, from_height, pixels, kind)
            .map_err(|err| err.to_string())?;
        let mut to = fir::images::Image::new(width, height, kind);
        let filter = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));
        Resizer::new()
            .resize(&from, &mut to, &filter)
            .map_err(|err| err.to_string())?;
        to.into_vec()
    };
    let size = [width as usize, height as usize];
    Ok(if kind == PixelType::U8x3 {
        ColorImage::from_rgb(size, &pixels)
    } else {
        ColorImage::from_rgba_unmultiplied(size, &pixels)
    })
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

fn checked_decoder(bytes: &[u8]) -> Result<impl ImageDecoder + '_, String> {
    let decoder = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?
        .into_decoder()
        .map_err(|err| format!("unsupported image: {err}"))?;
    let (width, height) = decoder.dimensions();
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        return Err(format!("image too large ({width}×{height})"));
    }
    Ok(decoder)
}

#[cfg(test)]
mod tests {

    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    fn check_dimensions(bytes: &[u8]) -> Result<(), String> {
        checked_decoder(bytes).map(drop)
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::GrayImage::new(width, height)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn decoded_images_are_dropped_once_uploaded() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let client = AddonClient::new(cineo_net::NetPolicy::default()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let loader =
            NetImageLoader::new(client, runtime.handle().clone(), ImageDisk::new(dir.path()));
        let cache = Arc::clone(&loader.cache);
        let uri = "https://img.example/poster.png";
        let image = ColorImage::filled([2, 2], egui::Color32::WHITE);
        cache
            .lock()
            .insert(uri.to_owned(), Poll::Ready(Ok(Arc::new(image))));
        let ctx = egui::Context::default();
        install(&ctx, loader);
        let poll = ctx
            .try_load_texture(uri, egui::TextureOptions::LINEAR, SizeHint::default())
            .unwrap();
        assert!(matches!(poll, egui::load::TexturePoll::Ready { .. }));
        assert!(cache.lock().is_empty());
    }

    fn held(entries: &[(&str, u64, usize)]) -> Vec<(String, u64, usize)> {
        entries
            .iter()
            .map(|&(uri, painted, bytes)| (uri.to_owned(), painted, bytes))
            .collect()
    }

    #[test]
    fn nothing_is_evicted_within_the_budget() {
        let entries = held(&[("a", 1, 10), ("b", 2, 10)]);
        assert!(over_budget(entries, 100, 20).is_empty());
    }

    #[test]
    fn the_least_recently_painted_go_first_and_only_until_it_fits() {
        let entries = held(&[("newest", 30, 10), ("oldest", 10, 10), ("middle", 20, 10)]);
        assert_eq!(over_budget(entries, 100, 20), vec!["oldest"]);
    }

    #[test]
    fn what_is_on_screen_is_never_evicted_even_over_the_budget() {
        let entries = held(&[("old", 10, 10), ("on screen", 99, 10), ("now", 100, 10)]);
        assert_eq!(over_budget(entries, 100, 0), vec!["old"]);
    }

    #[test]
    fn an_evicted_image_loses_its_texture_and_loads_again() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let client = AddonClient::new(cineo_net::NetPolicy::default()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let loader =
            NetImageLoader::new(client, runtime.handle().clone(), ImageDisk::new(dir.path()));
        let (cache, textures) = (Arc::clone(&loader.cache), Arc::clone(&loader.textures));
        let uri = "https://img.example/poster.png";
        let image = ColorImage::filled([2, 2], egui::Color32::WHITE);
        textures.lock().decoded(uri.to_owned(), image.size);
        cache
            .lock()
            .insert(uri.to_owned(), Poll::Ready(Ok(Arc::new(image))));
        let ctx = egui::Context::default();
        install(&ctx, loader);
        let load = || ctx.try_load_texture(uri, egui::TextureOptions::LINEAR, SizeHint::default());
        assert!(matches!(load(), Ok(egui::load::TexturePoll::Ready { .. })));
        assert!(
            textures.lock().held.contains_key(uri),
            "dropping the decoded copy keeps its size"
        );
        painted(&ctx, uri);
        evict_over(&ctx, 100, usize::MAX);
        assert!(matches!(load(), Ok(egui::load::TexturePoll::Ready { .. })));
        evict_over(&ctx, 100, 0);
        assert!(textures.lock().held.is_empty());
        assert!(matches!(
            load(),
            Ok(egui::load::TexturePoll::Pending { .. })
        ));
    }

    #[test]
    fn an_image_that_was_never_painted_is_not_counted() {
        let mut textures = Textures::default();
        textures.decoded("unpainted".to_owned(), [100, 100]);
        assert!(textures.take_over_budget(100, 0).is_empty());
        assert!(textures.held.contains_key("unpainted"));
    }

    const HINT: SizeHint = SizeHint::Size {
        width: 2,
        height: 2,
        maintain_aspect_ratio: true,
    };

    struct Rig {
        runtime: tokio::runtime::Runtime,
        ctx: egui::Context,
        dir: tempfile::TempDir,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                runtime: tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .unwrap(),
                ctx: egui::Context::default(),
                dir: tempfile::tempdir().unwrap(),
            }
        }

        fn disk(&self) -> ImageDisk {
            ImageDisk::new(self.dir.path())
        }

        fn serve(&self, body: Vec<u8>) -> (String, Arc<AtomicUsize>) {
            let listener = self
                .runtime
                .block_on(TcpListener::bind("127.0.0.1:0"))
                .unwrap();
            let address = listener.local_addr().unwrap();
            let served = Arc::new(AtomicUsize::new(0));
            let count = Arc::clone(&served);
            self.runtime.spawn(async move {
                loop {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    count.fetch_add(1, Ordering::SeqCst);
                    let mut request = [0; 1024];
                    let _ = socket.read(&mut request).await;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    socket.write_all(head.as_bytes()).await.unwrap();
                    socket.write_all(&body).await.unwrap();
                }
            });
            (format!("http://{address}/poster.png"), served)
        }

        fn load(&self, uri: &str, allow_private_networks: bool) -> Result<[usize; 2], String> {
            let client = AddonClient::new(cineo_net::NetPolicy {
                allow_private_networks,
                ..cineo_net::NetPolicy::default()
            })
            .unwrap();
            let loader = NetImageLoader::new(client, self.runtime.handle().clone(), self.disk());
            for _ in 0..500 {
                match loader.load(&self.ctx, uri, HINT) {
                    Ok(ImagePoll::Ready { image }) => return Ok(image.size),
                    Ok(ImagePoll::Pending { .. }) => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(err) => return Err(err.to_string()),
                }
            }
            panic!("the image never finished loading");
        }
    }

    #[test]
    fn an_image_on_disk_needs_no_network() {
        let rig = Rig::new();
        let uri = "http://127.0.0.1:9/poster.png";
        rig.disk().write(uri, &png(4, 4));
        assert_eq!(rig.load(uri, false), Ok([4, 4]));
    }

    #[test]
    fn a_fetched_image_is_saved_and_the_next_start_asks_nobody() {
        let rig = Rig::new();
        let (uri, served) = rig.serve(png(4, 4));
        assert_eq!(rig.load(&uri, true), Ok([4, 4]));
        assert_eq!(rig.load(&uri, true), Ok([4, 4]));
        assert_eq!(served.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn what_does_not_decode_is_not_saved() {
        let rig = Rig::new();
        let (uri, _) = rig.serve(b"not an image".to_vec());
        assert!(rig.load(&uri, true).is_err());
        assert_eq!(rig.disk().read(&uri), None);
    }

    #[test]
    fn a_corrupt_file_is_replaced_from_the_network() {
        let rig = Rig::new();
        let (uri, served) = rig.serve(png(4, 4));
        rig.disk().write(&uri, b"corrupt");
        assert_eq!(rig.load(&uri, true), Ok([4, 4]));
        assert_eq!(served.load(Ordering::SeqCst), 1);
        assert_eq!(rig.disk().read(&uri), Some(png(4, 4)));
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
    fn colour_and_transparency_survive_downscaling() {
        let mut out = Cursor::new(Vec::new());
        image::RgbImage::from_pixel(400, 600, image::Rgb([200, 40, 10]))
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let hint = SizeHint::Size {
            width: 50,
            height: 75,
            maintain_aspect_ratio: true,
        };
        let rgb = decode(&out.into_inner(), hint).unwrap();
        assert_eq!(rgb.size, [100, 150]);
        assert!(
            rgb.pixels
                .iter()
                .all(|p| *p == egui::Color32::from_rgb(200, 40, 10))
        );
        let mut out = Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(400, 600, image::Rgba([10, 200, 40, 0]))
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let clear = decode(&out.into_inner(), hint).unwrap();
        assert_eq!(clear.size, [100, 150]);
        assert!(clear.pixels.iter().all(|p| p.a() == 0));
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
