use std::sync::Arc;

use eframe::egui::{
    self, Color32, ColorImage, IconData, Rect, TextureHandle, TextureId, Ui, Vec2, pos2, vec2,
};

use crate::view::IMAGE_FILTER;

const LOGO: &[u8] = include_bytes!("../assets/brand/logo.png");
const ICON: &[u8] = include_bytes!("../assets/brand/icon.png");
const LOADING: [&[u8]; 5] = [
    include_bytes!("../assets/brand/loading/base.png"),
    include_bytes!("../assets/brand/loading/reel.png"),
    include_bytes!("../assets/brand/loading/moustache-left.png"),
    include_bytes!("../assets/brand/loading/moustache-right.png"),
    include_bytes!("../assets/brand/loading/strip.png"),
];

pub(crate) const LOADING_SIZE: Vec2 = vec2(328.0, 309.0);
const REEL_PIVOT: Vec2 = vec2(192.9, 192.0);
const REEL_PERIOD: f64 = 5.0;
const MOUSTACHE_PIVOT: Vec2 = vec2(135.0, 280.535);
const MOUSTACHE_PERIOD: f64 = 2.5;
const MOUSTACHE_KEYS: [(f64, f32); 6] = [
    (0.0, 0.0),
    (0.07, 5.0),
    (0.15, 0.0),
    (0.22, 5.0),
    (0.31, 0.0),
    (1.0, 0.0),
];
const STRIP_MIN: Vec2 = vec2(218.0, 134.0);
const STRIP_SIZE: Vec2 = vec2(108.0, 164.0);
const STRIP_FRAMES: usize = 10;
const STRIP_COLUMNS: usize = 5;
const STRIP_PERIOD: f64 = 1.0 / 3.0;

fn decode(bytes: &[u8]) -> Option<image::RgbaImage> {
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .ok()
        .map(|image| image.to_rgba8())
}

fn texture(
    ctx: &egui::Context,
    name: String,
    bytes: &[u8],
    options: egui::TextureOptions,
) -> Option<TextureHandle> {
    let image = decode(bytes)?;
    let size = [image.width() as usize, image.height() as usize];
    let pixels = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(name, pixels, options))
}

fn cached<T: Clone + Send + Sync + 'static>(
    ctx: &egui::Context,
    name: &str,
    make: impl FnOnce() -> Option<T>,
) -> Option<T> {
    let id = egui::Id::new(name);
    if let Some(value) = ctx.data(|d| d.get_temp::<T>(id)) {
        return Some(value);
    }
    let value = make()?;
    ctx.data_mut(|d| d.insert_temp(id, value.clone()));
    Some(value)
}

pub(crate) fn logo(ctx: &egui::Context) -> Option<TextureId> {
    cached(ctx, "cineo-logo", || {
        texture(ctx, "cineo-logo".into(), LOGO, egui::TextureOptions::LINEAR).map(Arc::new)
    })
    .map(|texture| texture.id())
}

/// Paints the animated logo filling `rect`, which should have [`LOADING_SIZE`]'s aspect.
pub(crate) fn paint_loading(ui: &Ui, rect: Rect, tint: Color32) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    ui.ctx().request_repaint();
    let layers = cached(ui.ctx(), "cineo-loading", || {
        LOADING
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                texture(
                    ui.ctx(),
                    format!("cineo-loading-{index}"),
                    bytes,
                    IMAGE_FILTER,
                )
            })
            .collect::<Option<Arc<[_]>>>()
    });
    let Some([base, reel, left, right, strip]) = layers.as_deref() else {
        return;
    };
    let time = ui.input(|i| i.time);
    let layer = |texture: &TextureHandle| egui::Image::from_texture(texture).tint(tint);
    layer(base).paint_at(ui, rect);
    layer(reel)
        .rotate(reel_angle(time), REEL_PIVOT / LOADING_SIZE)
        .paint_at(ui, rect);
    let wiggle = moustache_angle(time);
    for (texture, angle) in [(left, wiggle), (right, -wiggle)] {
        layer(texture)
            .rotate(angle, MOUSTACHE_PIVOT / LOADING_SIZE)
            .paint_at(ui, rect);
    }
    let frame = strip_frame(time);
    #[expect(clippy::cast_precision_loss, reason = "small frame counts")]
    let (column, row, cell) = (
        (frame % STRIP_COLUMNS) as f32,
        (frame / STRIP_COLUMNS) as f32,
        vec2(
            1.0 / STRIP_COLUMNS as f32,
            1.0 / STRIP_FRAMES.div_ceil(STRIP_COLUMNS) as f32,
        ),
    );
    let scale = rect.width() / LOADING_SIZE.x;
    layer(strip)
        .uv(Rect::from_min_size(
            pos2(column * cell.x, row * cell.y),
            cell,
        ))
        .paint_at(
            ui,
            Rect::from_min_size(rect.min + STRIP_MIN * scale, STRIP_SIZE * scale),
        );
}

fn reel_angle(time: f64) -> f32 {
    #[expect(clippy::cast_possible_truncation, reason = "an angle in 0..2π")]
    let angle = ((time / REEL_PERIOD).fract() * std::f64::consts::TAU) as f32;
    angle
}

fn moustache_angle(time: f64) -> f32 {
    let phase = (time / MOUSTACHE_PERIOD).fract();
    let degrees = MOUSTACHE_KEYS
        .windows(2)
        .find(|keys| phase < keys[1].0)
        .map_or(0.0, |keys| {
            let [(t0, from), (t1, to)] = [keys[0], keys[1]];
            #[expect(clippy::cast_possible_truncation, reason = "a factor in 0..1")]
            let u = ((phase - t0) / (t1 - t0)) as f32;
            from + (to - from) * u * u * (3.0 - 2.0 * u)
        });
    degrees.to_radians()
}

fn strip_frame(time: f64) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a frame index in 0..STRIP_FRAMES"
    )]
    let frame = ((time / STRIP_PERIOD).fract() * STRIP_FRAMES as f64) as usize;
    frame.min(STRIP_FRAMES - 1)
}

pub(crate) fn grain(ctx: &egui::Context, size: [usize; 2]) -> TextureId {
    let id = egui::Id::new("cineo-grain");
    if let Some((cached, texture)) =
        ctx.data(|d| d.get_temp::<([usize; 2], Arc<TextureHandle>)>(id))
        && cached == size
    {
        return texture.id();
    }
    let texture = ctx.load_texture(
        "cineo-grain",
        grain_image(size),
        egui::TextureOptions::NEAREST,
    );
    let texture_id = texture.id();
    ctx.data_mut(|d| d.insert_temp(id, (size, Arc::new(texture))));
    texture_id
}

fn grain_image([width, height]: [usize; 2]) -> ColorImage {
    let mut seed: u32 = 0x9e37_79b9;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let fade = fade(y as f32 / height.max(1) as f32);
        for x in 0..width {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed >> 24) as f32 / 127.5 - 1.0;
            let dx = x as f32 / width.max(1) as f32;
            let dy = y as f32 / height.max(1) as f32;
            let glow = (1.0 - (dx * dx + dy * dy).sqrt()).max(0.0) * GLOW;
            let light = (glow + noise.max(0.0) * GRAIN) * fade;
            let dark = (-noise).max(0.0) * GRAIN * fade;
            pixels.push(if light >= dark {
                egui::Color32::from_white_alpha(light.round() as u8)
            } else {
                egui::Color32::from_black_alpha(dark.round() as u8)
            });
        }
    }
    ColorImage::new([width, height], pixels)
}

/// How much of the sidebar backdrop shows at `t` (0 top, 1 bottom).
pub(crate) fn fade(t: f32) -> f32 {
    let fade = 1.0 - t.clamp(0.0, 1.0);
    fade * fade * (3.0 - 2.0 * fade)
}

const GRAIN: f32 = 14.0;
const GLOW: f32 = 10.0;

pub(crate) fn icon() -> Option<IconData> {
    let image = decode(ICON)?;
    Some(IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_and_icon_decode() {
        assert!(decode(LOGO).is_some());
        let icon = icon().expect("icon");
        assert_eq!((icon.width, icon.height), (256, 256));
    }

    #[test]
    fn the_loading_layers_share_the_canvas() {
        let [base, reel, left, right, strip] = LOADING.map(|bytes| decode(bytes).expect("layer"));
        for layer in [&reel, &left, &right] {
            assert_eq!(layer.dimensions(), base.dimensions());
        }
        assert_eq!(
            base.width() as f32 / base.height() as f32,
            LOADING_SIZE.x / LOADING_SIZE.y
        );
        assert_eq!(
            strip.dimensions(),
            (
                (STRIP_SIZE.x * 2.0) as u32 * STRIP_COLUMNS as u32,
                (STRIP_SIZE.y * 2.0) as u32 * STRIP_FRAMES.div_ceil(STRIP_COLUMNS) as u32
            )
        );
    }

    #[test]
    fn the_moustache_twitches_twice_then_rests() {
        let at = |phase: f64| moustache_angle(phase * MOUSTACHE_PERIOD).to_degrees();
        assert!(at(0.0).abs() < 1e-3);
        assert!((at(0.07) - 5.0).abs() < 1e-3);
        assert!(at(0.15).abs() < 1e-3);
        assert!((at(0.22) - 5.0).abs() < 1e-3);
        assert!(at(0.11) > 0.0 && at(0.11) < 5.0);
        assert!(at(0.5).abs() < 1e-3);
        assert!(at(0.99).abs() < 1e-3);
    }

    #[test]
    fn the_strip_cycles_through_every_frame() {
        let frames: Vec<usize> = (0..STRIP_FRAMES * 2)
            .map(|i| strip_frame((i as f64 + 0.5) * STRIP_PERIOD / STRIP_FRAMES as f64))
            .collect();
        let once: Vec<usize> = (0..STRIP_FRAMES).collect();
        assert_eq!(frames, [once.clone(), once].concat());
    }
}
