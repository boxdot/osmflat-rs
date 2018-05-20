extern crate prost_build;

fn main() {
    prost_build::compile_protos(
        &["src/proto/osmformat.proto", "src/proto/fileformat.proto"],
        &["src/proto"],
    ).expect("failed to compile protobuf");
}
