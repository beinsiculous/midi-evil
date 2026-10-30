use std::io::Cursor;

use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};

const MASCOT_PNG: &[u8] = include_bytes!("../assets/midi-evil-icon.png");
const SPLASH_PNG: &[u8] = include_bytes!("../assets/midi-evil-splash.png");
const CONTROLS_PNG: &[u8] = include_bytes!("../assets/maxwell-controls.png");

pub fn load_beats(context: &Context) -> TextureHandle {
    load_png(
        context,
        "maxwell-beats",
        include_bytes!("../assets/maxwell-beats.png"),
    )
}

pub fn load_keyboard(context: &Context) -> TextureHandle {
    load_png(
        context,
        "maxwell-keyboard",
        include_bytes!("../assets/maxwell-keyboard.png"),
    )
}

pub fn load_controls(context: &Context) -> TextureHandle {
    load_png(context, "maxwell-controls", CONTROLS_PNG)
}

pub fn control(texture: &TextureHandle, index: usize) -> eframe::egui::Image<'_> {
    use eframe::egui::{Image, Rect, pos2, vec2};
    Image::new((texture.id(), vec2(16.0, 16.0))).uv(Rect::from_min_max(
        pos2(index as f32 * 16.0 / texture.size()[0] as f32, 0.0),
        pos2((index + 1) as f32 * 16.0 / texture.size()[0] as f32, 1.0),
    ))
}

pub fn load_mascot(context: &Context) -> TextureHandle {
    load_png(context, "midi-evil-icon", MASCOT_PNG)
}

pub fn load_splash(context: &Context) -> TextureHandle {
    load_png(context, "midi-evil-splash", SPLASH_PNG)
}

fn load_png(context: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("embedded branding PNG is valid");
    let mut data = vec![
        0;
        reader
            .output_buffer_size()
            .expect("branding has a known size")
    ];
    let frame = reader
        .next_frame(&mut data)
        .expect("embedded branding PNG can be decoded");
    let data = &data[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        format => panic!("unsupported embedded branding color type: {format:?}"),
    };
    let image =
        ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &rgba);
    context.load_texture(name, image, TextureOptions::NEAREST)
}
