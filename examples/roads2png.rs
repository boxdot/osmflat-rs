extern crate bresenham;
extern crate failure;
extern crate flatdata;
extern crate itertools;
extern crate osmflat;
extern crate png;
extern crate structopt;

use bresenham::Bresenham;
use failure::Error;
use flatdata::{Archive, FileResourceStorage};
use itertools::Itertools;
use png::HasParameters;
use structopt::StructOpt;

use std::convert;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use std::str;

#[derive(Debug, Clone, Copy)]
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

impl<'a> convert::From<osmflat::RefNode<'a>> for GeoCoord {
    fn from(node: osmflat::RefNode<'a>) -> Self {
        const COORD_SCALE: f64 = 0.000000001;
        Self {
            lat: node.lat() as f64 * COORD_SCALE,
            lon: node.lon() as f64 * COORD_SCALE,
        }
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
            data: vec![255; (w * h) as usize],
        }
    }

    fn set_black(&mut self, x: u32, y: u32) {
        self.data[(y * self.w + x) as usize] = 0;
    }
}

#[derive(Debug, Clone)]
struct MapTransform {
    min_x: f64,
    max_y: f64,
    scale_x: f64,
    scale_y: f64,
}

impl MapTransform {
    fn new(width: u32, height: u32, min: GeoCoord, max: GeoCoord) -> Self {
        Self {
            min_x: min.lon,
            max_y: max.lat,
            scale_x: (max.lon - min.lon) / (width as f64),
            scale_y: (max.lat - min.lat) / (height as f64),
        }
    }

    fn transform(&self, coord: GeoCoord) -> (isize, isize) {
        (
            ((coord.lon - self.min_x) / self.scale_x) as isize,
            ((self.max_y - coord.lat) / self.scale_y) as isize,
        )
    }
}

fn way_coords<'a>(
    archive: &'a osmflat::Osm,
    way: &osmflat::RefWay,
    next_way: &osmflat::RefWay,
) -> impl Iterator<Item = GeoCoord> + 'a {
    let nodes = archive.nodes();
    let nodes_index = archive.nodes_index();
    let begin = way.ref_first_idx() as usize;
    let end = next_way.ref_first_idx() as usize;
    (begin..end)
        .map(move |i| nodes.at(nodes_index.at(i).value() as usize))
        .map(GeoCoord::from)
}

fn substring(strings: &str, start: u64) -> &str {
    let start = start as usize;
    let end = strings[start..].find('\0').expect("invalid string");
    &strings[start..start + end]
}

fn way_filter(
    way: &osmflat::RefWay,
    next_way: &osmflat::RefWay,
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

    let unwanted_highway_types = [
        "pedestrian",
        "steps",
        "footway",
        "construction",
        "bic",
        "cycleway",
        "layby",
        "bridleway",
        "path",
    ];

    // Filter all ways that do not have desirable highway tag.
    let start_tag_idx = way.tag_first_idx();
    let end_tag_idx = next_way.tag_first_idx();
    (start_tag_idx..end_tag_idx).any(|tag_idx| {
        let tag = tags.at(tags_index.at(tag_idx as usize).value() as usize);
        let key = substring(strings, tag.key_idx());
        if key == "highway" {
            let val = substring(strings, tag.value_idx());
            !unwanted_highway_types.contains(&val)
        } else {
            false
        }
    })
}

fn roads<'a>(
    archive: &'a osmflat::Osm,
) -> impl Iterator<Item = (osmflat::RefWay, osmflat::RefWay)> {
    let ways = archive.ways();
    let tags_index = archive.tags_index();
    let tags = archive.tags();
    // we checked strings in the beginning, it's safe to do 0-overhead retrieval.
    let strings = unsafe { str::from_utf8_unchecked(archive.stringtable()) };

    ways.iter()
        .tuple_windows()
        .filter(move |(way, next_way)| way_filter(&way, &next_way, &tags_index, &tags, strings))
}

fn render(archive: &osmflat::Osm, width: u32) -> Image {
    str::from_utf8(archive.stringtable()).expect("stringtable contains invalid utf8 characters");

    // compute extent
    let mut coords =
        roads(archive).flat_map(|(way, next_way)| way_coords(archive, &way, &next_way));
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

    let lines = roads(archive).flat_map(|(way, next_way)| {
        way_coords(archive, &way, &next_way)
            .map(|coord| t.transform(coord))
            .tuple_windows()
    });

    for (from, to) in lines {
        for (x, y) in Bresenham::new(from, to) {
            image.set_black(x as u32, y as u32);
        }
    }

    image
}

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: PathBuf,

    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,

    #[structopt(short = "w", long = "width", default_value = "4320")]
    width: u32,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    let storage = FileResourceStorage::new(opt.input);
    let archive = osmflat::Osm::open(storage)?;

    let image = render(&archive, opt.width);

    let path = Path::new(&opt.output);
    let file = File::create(path)?;
    let buf = BufWriter::new(file);

    let mut encoder = png::Encoder::new(buf, image.w, image.h);
    encoder
        .set(png::ColorType::Grayscale)
        .set(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;

    writer.write_image_data(&image.data[..])?;
    Ok(())
}
