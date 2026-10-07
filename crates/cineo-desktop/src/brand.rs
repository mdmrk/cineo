//! Cineo's own artwork, embedded in the binary.

use eframe::egui::{self, ColorImage, IconData, TextureHandle};

const LOGO: &[u8] = include_bytes!("../assets/brand/logo.png");
const ICON: &[u8] = include_bytes!("../assets/brand/icon.png");

fn decode(bytes: &[u8]) -> Option<image::RgbaImage> {
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .ok()
        .map(|image| image.to_rgba8())
}

pub(crate) fn logo(ctx: &egui::Context) -> Option<TextureHandle> {
    let id = egui::Id::new("cineo-logo");
    if let Some(texture) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(texture);
    }
    let image = decode(LOGO)?;
    let size = [image.width() as usize, image.height() as usize];
    let texture = ctx.load_texture(
        "cineo-logo",
        ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

pub(crate) fn grain(ctx: &egui::Context, size: [usize; 2]) -> TextureHandle {
    let id = egui::Id::new("cineo-grain");
    if let Some((cached, texture)) = ctx.data(|d| d.get_temp::<([usize; 2], TextureHandle)>(id))
        && cached == size
    {
        return texture;
    }
    let texture = ctx.load_texture(
        "cineo-grain",
        grain_image(size),
        egui::TextureOptions::NEAREST,
    );
    ctx.data_mut(|d| d.insert_temp(id, (size, texture.clone())));
    texture
}

fn grain_image([width, height]: [usize; 2]) -> ColorImage {
    let mut seed: u32 = 0x9e37_79b9;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let fade = 1.0 - y as f32 / height.max(1) as f32;
        let fade = fade * fade * (3.0 - 2.0 * fade);
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
    fn grain_varies_and_fades_out_at_the_bottom() {
        let image = grain_image([40, 30]);
        assert_eq!(image, grain_image([40, 30]), "deterministic");
        let top: Vec<_> = image.pixels[..40].iter().map(|p| p.to_array()).collect();
        assert!(top.windows(2).any(|w| w[0] != w[1]), "grain");
        let bottom = &image.pixels[29 * 40..];
        assert!(bottom.iter().all(|p| p.a() <= 1), "fades out");
    }

    #[test]
    fn the_logo_and_icon_decode() {
        assert!(decode(LOGO).is_some());
        let icon = icon().expect("icon");
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}
