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

#[derive(Debug, Clone, Copy, Default)]
struct GeoCoord {
    lat: f64,
    lon: f64,
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

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn width(&self) -> u32 {
        self.w
    }

    fn height(&self) -> u32 {
        self.h
    }
}

#[derive(Debug)]
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
        let res = (
            ((coord.lon - self.min_x) / self.map_w * self.width as f64) as isize,
            ((1f64 - (coord.lat - self.min_y) / self.map_h) * self.height as f64) as isize,
        );
        res
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

fn render(archive: &osmflat::Osm, image: &mut Image) {
    let nodes = archive.nodes();
    let nodes_index = archive.nodes_index();

    let ways = archive.ways();
    let tags_index = archive.tags_index();
    let tags = archive.tags();
    let strings = str::from_utf8(archive.stringtable())
        .expect("stringtable contains invalid utf8 characters");

    let get_roads = || {
        ways.iter()
            .zip(ways.iter().skip(1))
            .filter(|(way, next_way)| way_filter(&*way, &*next_way, &tags_index, &tags, strings))
    };

    // compute extent

    let mut roads = get_roads();
    let (road, _) = roads.next().expect("no roads found");
    let first_road_node = nodes.at(nodes_index.at(road.ref_first_idx() as usize).value() as usize);
    let mut min = GeoCoord::from(first_road_node);
    let mut max = min;

    for (way, next_way) in get_roads() {
        let start_node_idx = way.ref_first_idx();
        let end_node_idx = next_way.ref_first_idx();
        for node_idx in start_node_idx..end_node_idx {
            let node = nodes.at(nodes_index.at(node_idx as usize).value() as usize);
            let coord = GeoCoord::from(node);
            if coord.lon < min.lon {
                min.lon = coord.lon;
            } else if max.lon < coord.lon {
                max.lon = coord.lon;
            }
            if coord.lat < min.lat {
                min.lat = coord.lat;
            } else if max.lat < coord.lat {
                max.lat = coord.lat;
            }
        }
    }

    let t = MapTransform::new(image.width() - 1, image.height() - 1, min, max);

    for (way, next_way) in get_roads() {
        let start_node_idx = way.ref_first_idx();
        let end_node_idx = next_way.ref_first_idx();
        for node_idx in start_node_idx..(end_node_idx - 1) {
            let from = nodes.at(nodes_index.at(node_idx as usize).value() as usize);
            let to = nodes.at(nodes_index.at((node_idx + 1) as usize).value() as usize);

            let from_raster = t.transform(GeoCoord::from(from));
            let to_raster = t.transform(GeoCoord::from(to));

            for (x, y) in Bresenham::new(from_raster, to_raster) {
                image.set(x as u32, y as u32, Color::new(0, 0, 0, 255));
            }
        }
    }
}

fn main() -> Result<(), Error> {
    let mut args = env::args().skip(1);
    let osmflat_path = args.next().unwrap();
    let image_path = args.next().unwrap();

    let storage = Rc::new(RefCell::new(FileResourceStorage::new(osmflat_path.into())));
    let archive = osmflat::Osm::open(storage)?;

    let mut image = Image::new(4 * 1000, 4 * 800);
    render(&archive, &mut image);

    let path = Path::new(&image_path);
    let file = File::create(path)?;
    let buf = BufWriter::new(file);

    let mut encoder = png::Encoder::new(buf, 4 * 1000, 4 * 800);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;

    writer.write_image_data(image.data())?;
    Ok(())
}
