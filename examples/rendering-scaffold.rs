extern crate flatdata;
extern crate osmflat;

use flatdata::Archive;

use std::error::Error;
use std::ops::Range;

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
struct GeoCoord {
    lat: f64,
    lng: f64,
}

impl<'a> From<osmflat::RefNode<'a>> for GeoCoord {
    fn from(node: osmflat::RefNode<'a>) -> Self {
        const COORD_SCALE: f64 = 0.000000001;
        Self {
            lat: node.lat() as f64 * COORD_SCALE,
            lng: node.lon() as f64 * COORD_SCALE,
        }
    }
}

struct Polyline {
    inner: Vec<Range<u64>>,
}

impl From<Range<u64>> for Polyline {
    fn from(range: Range<u64>) -> Self {
        Self { inner: vec![range] }
    }
}

impl Polyline {
    fn into_iter(self, archive: osmflat::Osm) -> PolylineIter {
        let next_element = self.inner.first().map(|range| range.start).unwrap_or(0);
        PolylineIter {
            archive,
            inner: self.inner,
            next_range: 0,
            next_element,
        }
    }
}

struct PolylineIter {
    archive: osmflat::Osm,
    inner: Vec<Range<u64>>,
    next_range: usize,
    next_element: u64,
}

impl Iterator for PolylineIter {
    type Item = GeoCoord;
    fn next(&mut self) -> Option<GeoCoord> {
        if self.next_range < self.inner.len() {
            let range = &self.inner[self.next_range];
            if self.next_element < range.end {
                let node_idx = self
                    .archive
                    .nodes_index()
                    .at(self.next_element as usize)
                    .value();
                let coord = self.archive.nodes().at(node_idx as usize).into();
                self.next_element += 1;
                if self.next_element == range.end {
                    self.next_range += 1;
                    self.next_element = self
                        .inner
                        .get(self.next_range)
                        .map(|range| range.start)
                        .unwrap_or(0);
                }
                Some(coord)
            } else {
                None
            }
        } else {
            None
        }
    }
}

// scaffold

fn classify(archive: osmflat::Osm) -> impl Iterator<Item = Feature> {
    let inner_archive = archive.clone();
    let ways = (0..archive.ways().len() as u64 - 2).filter_map(move |idx| {
        classify_way(inner_archive.clone(), idx).map(|cat| Feature { idx, cat })
    });
    let rels = (0..archive.relations().len() as u64 - 2).filter_map(move |idx| {
        classify_relation(archive.clone(), idx).map(|cat| Feature { idx, cat })
    });
    ways.chain(rels)
}

fn classify_way(_archive: osmflat::Osm, _idx: u64) -> Option<Category> {
    None
}

fn classify_relation(_archive: osmflat::Osm, _idx: u64) -> Option<Category> {
    None
}

enum Category {
    Road,
    Park,
    River,
    Water,
}

struct Feature {
    idx: u64,
    cat: Category,
}

impl Feature {
    fn into_polyline(self, archive: osmflat::Osm) -> Polyline {
        match self.cat {
            Category::Road | Category::River => way_into_polyline(archive, self.idx),
            Category::Park | Category::Water => multipolygon_into_polyline(archive, self.idx),
        }
    }
}

fn way_into_polyline(_archive: osmflat::Osm, _idx: u64) -> Polyline {
    unimplemented!()
}

fn multipolygon_into_polyline(_archive: osmflat::Osm, _idx: u64) -> Polyline {
    unimplemented!()
}

fn render(archive: osmflat::Osm, polyline: Polyline) {
    let encoded: Vec<_> = polyline
        .into_iter(archive)
        .map(|coord| format!("{},{}", coord.lat, coord.lng))
        .collect();
    println!("{}", encoded.join(","));
}

fn main() -> Result<(), Box<Error>> {
    let storage = flatdata::FileResourceStorage::new("berlin.flatdata".into());
    let archive = osmflat::Osm::open(storage)?;

    let features = classify(archive.clone());
    let archive_inner = archive.clone();
    let polylines = features.map(move |f| f.into_polyline(archive_inner.clone()));
    polylines.for_each(move |p| render(archive.clone(), p));

    Ok(())
}
