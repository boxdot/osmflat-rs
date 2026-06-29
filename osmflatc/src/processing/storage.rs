use super::{Key, Value};

#[derive(Debug, Clone, Copy)]
pub struct OsmKey {
    pub spatial_index: u64,
    pub id: i64,
}

impl OsmKey {
    pub fn new(spatial_index: u64, id: i64) -> Self {
        Self { spatial_index, id }
    }
}

impl From<Box<[u8]>> for OsmKey {
    fn from(bytes: Box<[u8]>) -> Self {
        let bytes = &bytes[..];
        assert!(bytes.len() == 16, "Key bytes were unexpected length");
        let spatial_index = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let id = i64::from_be_bytes(bytes[8..16].try_into().unwrap());
        Self { spatial_index, id }
    }
}

impl Key for OsmKey {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16);
        out.extend(self.spatial_index.to_be_bytes());
        out.extend(self.id.to_be_bytes());
        out
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct OsmIdKey {
    pub id: i64,
}

impl OsmIdKey {
    pub fn new(id: i64) -> Self {
        OsmIdKey { id }
    }
}

impl Key for OsmIdKey {
    fn serialize(&self) -> Vec<u8> {
        self.id.to_be_bytes().to_vec()
    }
}

impl From<Box<[u8]>> for OsmIdKey {
    fn from(value: Box<[u8]>) -> Self {
        let id = i64::from_be_bytes(value[0..8].try_into().unwrap());
        OsmIdKey { id }
    }
}

pub struct OsmIdxValue {
    pub idx: u64,
}

impl OsmIdxValue {
    pub fn new(idx: u64) -> Self {
        OsmIdxValue { idx }
    }
}

impl Value for OsmIdxValue {
    fn serialize(&self) -> Vec<u8> {
        self.idx.to_be_bytes().to_vec()
    }
}

impl From<Box<[u8]>> for OsmIdxValue {
    fn from(value: Box<[u8]>) -> Self {
        OsmIdxValue {
            idx: u64::from_be_bytes(value[0..8].try_into().unwrap()),
        }
    }
}
