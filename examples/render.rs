extern crate png;

use png::HasParameters;

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;

#[derive(Debug, Default, Clone, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug)]
struct Image {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

impl Image {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            data: vec![255; (w * h) as usize * 4],
        }
    }

    fn set(&mut self, x: u32, y: u32, c: Color) {
        let i = (y * self.w + x) as usize * 4;
        self.data[i + 0] = c.r;
        self.data[i + 1] = c.g;
        self.data[i + 2] = c.b;
        self.data[i + 3] = c.a;
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

/// Bresenham's line algorithm
fn bresenham<F: FnMut(i32, i32) -> ()>(
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    mut set_pixel: F,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0);
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };

    let mut err = dx + dy;
    let mut e2;

    loop {
        set_pixel(x0, y0);
        if x0 == x1 && y0 == y1 {
            break;
        }
        e2 = 2 * err;
        if e2 > dy {
            // e_xy+e_x > 0
            err += dy;
            x0 += sx;
        }
        if e2 < dx {
            // e_xy+e_y < 0
            err += dx;
            y0 += sy;
        }
    }
}

fn main() -> Result<(), io::Error> {
    let mut image = Image::new(100, 100);
    bresenham(0, 0, 99, 99, |x, y| {
        image.set(x as u32, y as u32, Color::new(0, 0, 0, 255));
    });

    let path = Path::new("image.png");
    let file = File::create(path)?;
    let buf = BufWriter::new(file);

    let mut encoder = png::Encoder::new(buf, 100, 100);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;

    writer.write_image_data(image.data())?;
    Ok(())
}
