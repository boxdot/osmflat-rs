# osmflat ![][ci]

![berlin-features]

Flat OpenStreetMap (OSM) data format providing an efficient *random* data
access through [memory mapped files].

The data format is described and implemented in [flatdata]. The [schema]
describes the fundamental OSM data structures: nodes, ways, relations and
tags as simple non-nested data structures. The relations between these are
expressed through indexes.

## Compiler

Besides the library for working with osmflat archives, the crate `osmflatc`
contains an OSM [pbf format][PBF format] to osmflat data compiler.

To compile OSM data from pbf to osmflat use:

```shell
cargo run --release -- input.osm.pbf output.osm.flatdata
```

The output is a flatdata which is a directory consisting of several
files. The schema is also part of the archive. It is checked every time the
archive is opened. This guarantees that the compiler which was used to produce
the archive fits to the schema used for reading it. The archive data is not
compressed.

## Using data

You can use any [flatdata] supported language for reading an osmflat archive.
For reading the data in Rust, we provide the `osmflat` crate.

First, add this to your Cargo.toml:

```toml
[dependencies]
osmflat = "0.3.0"
```

Now, you can open an osmflat archive as any other flatdata archive and read its
data:

```rust
use osmflat::{FileResourceStorage, Osm};

fn main() {
    let storage = FileResourceStorage::new("path/to/archive.osm.flatdata");
    let archive = Osm::open(storage).unwrap();

    for node in archive.nodes().iter() {
        println!("{:?}", node);
    }
}
```

## Examples

Check the [osmflat/examples] directory. Feel free to add another example, if
you have an idea what to do with the amazing OSM data in few lines of code. 😁

The above map was rendered by `osmflat/examples/roads2png.rs` in ~ 170 loc from
the osmflat archive based on the [latest][latest-berlin-map] Berlin OSM data.

## License

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT License ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

The files [src/proto/fileformat.proto](src/proto/fileformat.proto) and
[src/proto/osmformat.proto](src/proto/osmformat.proto) are copies from the
[OSM-binary] project and are under the LGPLv3 license.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this document by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[flatdata]: https://github.com/heremaps/flatdata
[schema]: flatdata/osm.flatdata
[memory mapped files]: https://en.wikipedia.org/wiki/Memory-mapped_file
[PBF format]: https://wiki.openstreetmap.org/wiki/PBF_Format
[osmflat/examples]: osmflat/examples
[latest-berlin-map]: http://download.geofabrik.de/europe/germany/berlin.html
[OSM-binary]: https://github.com/scrosby/OSM-binary
[ci]: https://github.com/boxdot/osmflat-rs/workflows/ci/badge.svg
[berlin-features]: https://github.com/boxdot/osmflat-rs/blob/master/osmflat/examples/berlin-features.png
