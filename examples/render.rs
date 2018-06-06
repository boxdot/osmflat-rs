extern crate bresenham;
extern crate failure;
extern crate flatdata;
extern crate osmflat;
extern crate png;

use bresenham::Bresenham;
use failure::Error;
use flatdata::{Archive, FileResourceStorage};
use png::HasParameters;

use std::cell::RefCell;
use std::convert;
use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;
use std::str;

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
struct GeoCoord {
    lat: f64,
    lon: f64,
}

impl GeoCoord {
    fn min(self, other: Self) -> Self {
        Self {
            lat: self.lat.min(other.lat),
            lon: self.lon.min(other.lon),
        }
    }

    fn max(self, other: Self) -> Self {
        Self {
            lat: self.lat.max(other.lat),
            lon: self.lon.max(other.lon),
        }
    }
}

impl<N: Deref<Target = osmflat::Node>> convert::From<N> for GeoCoord {
    fn from(node: N) -> Self {
        const COORD_SCALE: f64 = 0.000000001;
        Self {
            lat: node.lat() as f64 * COORD_SCALE,
            lon: node.lon() as f64 * COORD_SCALE,
        }
    }
}

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
}

#[derive(Debug, Clone)]
struct MapTransform {
    width: u32,
    height: u32,
    min_x: f64,
    min_y: f64,
    map_w: f64,
    map_h: f64,
}

impl MapTransform {
    fn new(width: u32, height: u32, min: GeoCoord, max: GeoCoord) -> Self {
        Self {
            width,
            height,
            min_x: min.lon,
            min_y: min.lat,
            map_w: max.lon - min.lon,
            map_h: max.lat - min.lat,
        }
    }

    fn transform(&self, coord: GeoCoord) -> (isize, isize) {
        (
            ((coord.lon - self.min_x) / self.map_w * self.width as f64) as isize,
            ((1f64 - (coord.lat - self.min_y) / self.map_h) * self.height as f64) as isize,
        )
    }
}

#[derive(Clone)]
struct NodesIterator<'a> {
    nodes: flatdata::ArrayView<'a, osmflat::Node>,
    nodes_index: flatdata::ArrayView<'a, osmflat::NodeIndex>,
    next: usize,
    end: usize,
}

impl<'a> NodesIterator<'a> {
    fn from_way(archive: &'a osmflat::Osm, way: &osmflat::Way, next_way: &osmflat::Way) -> Self {
        Self {
            nodes: archive.nodes(),
            nodes_index: archive.nodes_index(),
            next: way.ref_first_idx() as usize,
            end: next_way.ref_first_idx() as usize,
        }
    }
}

impl<'a> Iterator for NodesIterator<'a> {
    type Item = flatdata::Handle<'a, osmflat::Node>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.end {
            let idx = self.next;
            self.next += 1;
            Some(self.nodes.at(self.nodes_index.at(idx).value() as usize))
        } else {
            None
        }
    }
}

fn substring(strings: &str, start: u32) -> &str {
    let start = start as usize;
    let end = strings[start..].find('\0').expect("invalid string");
    &strings[start..start + end]
}

fn way_filter(
    way: &osmflat::Way,
    next_way: &osmflat::Way,
    tags_index: &flatdata::ArrayView<osmflat::TagIndex>,
    tags: &flatdata::ArrayView<osmflat::Tag>,
    strings: &str,
) -> bool {
    // Filter all ways that have less than 2 nodes.
    let start_node_idx = way.ref_first_idx();
    let end_node_idx = next_way.ref_first_idx();
    if end_node_idx - start_node_idx < 2 {
        return false;
    }

    // Filter all ways that do not have a highway tag. Also check for specific
    // values.
    let start_tag_idx = way.tag_first_idx();
    let end_tag_idx = next_way.tag_first_idx();
    for tag_idx in start_tag_idx..end_tag_idx {
        let tag = tags.at(tags_index.at(tag_idx as usize).value() as usize);
        let key = substring(strings, tag.key_idx());
        if key == "highway" {
            let val = substring(strings, tag.value_idx());
            if val == "pedestrian" || val == "steps" || val == "footway" || val == "construction"
                || val == "bic" || val == "cycleway" || val == "layby"
                || val == "bridleway" || val == "path"
            {
                return false;
            }
            return true;
        }
    }

    false
}

fn render(archive: &osmflat::Osm, width: u32) -> Image {
    let ways = archive.ways();
    let tags_index = archive.tags_index();
    let tags = archive.tags();
    let strings = str::from_utf8(archive.stringtable())
        .expect("stringtable contains invalid utf8 characters");

    let roads = ways.iter()
        .zip(ways.iter().skip(1))
        .filter(|(way, next_way)| way_filter(&*way, &*next_way, &tags_index, &tags, strings));

    // compute extent
    let mut coords = roads.clone().flat_map(|(way, next_way)| {
        NodesIterator::from_way(archive, &*way, &*next_way).map(GeoCoord::from)
    });
    let first_coord = coords.next().expect("no roads found");
    let (min, max) = coords.fold((first_coord, first_coord), |(min, max), coord| {
        (min.min(coord), max.max(coord))
    });

    // compute ratio and height
    let ratio = 360. / 180. * (max.lat - min.lat) / (max.lon - min.lon);
    let height = (width as f64 * ratio) as u32;

    // create world -> raster transformation
    let t = MapTransform::new(width - 1, height - 1, min, max);

    // draw
    let mut image = Image::new(width, height);

    let lines = roads.flat_map(|(way, next_way)| {
        let raster_coords = NodesIterator::from_way(archive, &*way, &*next_way)
            .map(GeoCoord::from)
            .map(|coord| t.transform(coord));
        raster_coords.clone().zip(raster_coords.skip(1))
    });

    for (from, to) in lines {
        for (x, y) in Bresenham::new(from, to) {
            image.set(x as u32, y as u32, Color::new(0, 0, 0, 255));
        }
    }

    image
}

fn main() -> Result<(), Error> {
    // Lets do 4k :D
    const WIDTH: u32 = 4 * 1080;

    let mut args = env::args().skip(1);
    let osmflat_path = args.next().unwrap();
    let image_path = args.next().unwrap();

    let storage = Rc::new(RefCell::new(FileResourceStorage::new(osmflat_path.into())));
    let archive = osmflat::Osm::open(storage)?;

    let image = render(&archive, WIDTH);

    let path = Path::new(&image_path);
    let file = File::create(path)?;
    let buf = BufWriter::new(file);

    let mut encoder = png::Encoder::new(buf, WIDTH, image.h);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;

    writer.write_image_data(&image.data[..])?;
    Ok(())
}
