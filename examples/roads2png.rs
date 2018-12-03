extern crate bresenham;
extern crate failure;
extern crate itertools;
extern crate osmflat;
extern crate png;
extern crate structopt;

use bresenham::Bresenham;
use failure::Error;
use itertools::Itertools;
use osmflat::*;
use png::HasParameters;
use structopt::StructOpt;

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use std::str;

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

fn compute_bounds<'a>(mut iter: impl Iterator<Item = RefNode<'a>>) -> ((i64, i64), (i64, i64)) {
    let first_coord = iter.next().map(|c| (c.lat(), c.lon())).unwrap_or((0, 0));
    iter.fold((first_coord, first_coord), |(min, max), coord| {
        (
            (min.0.min(coord.lat()), min.1.min(coord.lon())),
            (max.0.max(coord.lat()), max.1.max(coord.lon())),
        )
    })
}

fn map_transform<'a>(
    width: u32,
    height: u32,
    min: (i64, i64),
    max: (i64, i64),
) -> impl FnMut(RefNode<'a>) -> (isize, isize) + Copy {
    move |coord: RefNode<'a>| {
        (
            ((coord.lon() - min.1) as f64 / (max.1 - min.1) as f64 * width as f64) as isize,
            ((max.0 - coord.lat()) as f64 / (max.0 - min.0) as f64 * height as f64) as isize,
        )
    }
}

fn way_nodes<'a>(
    archive: &'a osmflat::Osm,
    way: &osmflat::RefWay,
    next_way: &osmflat::RefWay,
) -> impl Iterator<Item = RefNode<'a>> + 'a {
    let nodes = archive.nodes();
    let nodes_index = archive.nodes_index();
    let begin = way.ref_first_idx() as usize;
    let end = next_way.ref_first_idx() as usize;
    (begin..end).map(move |i| nodes.at(nodes_index.at(i).value() as usize))
}

fn substring(strings: &[u8], start: usize) -> &str {
    let end = strings[start..]
        .iter()
        .position(|&c| c == 0)
        .expect("invalid string");
    std::str::from_utf8(&strings[start..start + end]).expect("invalid string")
}

fn way_filter(
    way: &osmflat::RefWay,
    next_way: &osmflat::RefWay,
    tags_index: &flatdata::ArrayView<osmflat::TagIndex>,
    tags: &flatdata::ArrayView<osmflat::Tag>,
    strings: &[u8],
) -> bool {
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
        let key = substring(strings, tag.key_idx() as usize);
        let val = substring(strings, tag.value_idx() as usize);
        key == "highway" && !unwanted_highway_types.contains(&val)
    })
}

fn roads<'a>(
    archive: &'a osmflat::Osm,
) -> impl Iterator<Item = (osmflat::RefWay, osmflat::RefWay)> {
    let ways = archive.ways();
    let tags_index = archive.tags_index();
    let tags = archive.tags();
    let strings = archive.stringtable();

    ways.iter()
        .tuple_windows()
        .filter(move |(way, next_way)| way_filter(&way, &next_way, &tags_index, &tags, strings))
}

fn render(archive: &osmflat::Osm, width: u32) -> Image {
    // compute extent
    let coords = roads(archive).flat_map(|(way, next_way)| way_nodes(archive, &way, &next_way));
    let (min, max) = compute_bounds(coords);

    // compute ratio and height
    let ratio = 360. / 180. * (max.0 - min.0) as f64 / (max.1 - min.1) as f64;
    let height = (width as f64 * ratio) as u32;

    // create world -> raster transformation
    let t = map_transform(width - 1, height - 1, min, max);

    // draw
    let mut image = Image::new(width, height);

    let line_segments = roads(archive)
        .flat_map(|(way, next_way)| way_nodes(archive, &way, &next_way).map(t).tuple_windows());

    line_segments
        .flat_map(|(from, to)| Bresenham::new(from, to))
        .for_each(|(x, y)| image.set_black(x as u32, y as u32));

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

    let archive = Osm::open(FileResourceStorage::new(opt.input))?;

    let image = render(&archive, opt.width);

    let buf = BufWriter::new(File::create(&opt.output)?);
    let mut encoder = png::Encoder::new(buf, image.w, image.h);
    encoder
        .set(png::ColorType::Grayscale)
        .set(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data[..])?;

    Ok(())
}
