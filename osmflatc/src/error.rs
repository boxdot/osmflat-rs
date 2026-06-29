use thiserror::Error;

#[derive(Error, Debug)]
pub enum OsmFlatcError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protobuf Error: {0}")]
    Prost(#[from] prost::DecodeError),

    #[error("Database Error: {0}")]
    RocksDB(#[from] rocksdb::Error),

    #[error("UTF-8 Error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}
