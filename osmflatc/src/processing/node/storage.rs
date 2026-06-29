use crate::processing::storage::OsmIdKey;
use crate::processing::storage::OsmIdxValue;
use crate::processing::storage::OsmKey;
use crate::processing::TempDataCodec;
use crate::processing::Value;

pub struct NodeValue {
    pub lon: i32,
    pub lat: i32,
    pub refs: Vec<(u64, u64)>,
}

impl NodeValue {
    pub fn new(lon: i32, lat: i32, refs: Vec<(u64, u64)>) -> Self {
        Self { lon, lat, refs }
    }
}

impl From<Box<[u8]>> for NodeValue {
    fn from(bytes: Box<[u8]>) -> Self {
        // Layout: lon (i32 LE), lat (i32 LE), then 16-byte (key, value) ref pairs.
        // Coordinates are stored inline so the spatial-ordering pass can read
        // them straight from this sequential scan instead of doing a random
        // `NodeIdToLonLat` lookup per node.
        let lon = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let lat = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let refs = bytes[8..]
            .chunks(16)
            .map(|chunk| {
                let key = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
                let value = u64::from_le_bytes(chunk[8..16].try_into().unwrap());
                (key, value)
            })
            .collect();
        Self { lon, lat, refs }
    }
}

impl Value for NodeValue {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 16 * self.refs.len());
        out.extend(&self.lon.to_le_bytes());
        out.extend(&self.lat.to_le_bytes());
        for &(key, value) in &self.refs {
            out.extend(&key.to_le_bytes());
            out.extend(&value.to_le_bytes());
        }
        out
    }
}

pub struct NodesTDC;

impl TempDataCodec for NodesTDC {
    type Key = OsmKey;
    type Value = NodeValue;

    const NAME: &'static str = "NODES";
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct NodeLonLatValue {
    pub lon: i32,
    pub lat: i32,
}

impl NodeLonLatValue {
    pub fn new(lon: i32, lat: i32) -> Self {
        NodeLonLatValue { lon, lat }
    }
}

impl Value for NodeLonLatValue {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&self.lon.to_be_bytes());
        out.extend_from_slice(&self.lat.to_be_bytes());
        out
    }
}

impl From<Box<[u8]>> for NodeLonLatValue {
    fn from(value: Box<[u8]>) -> Self {
        let lon = i32::from_be_bytes(value[0..4].try_into().unwrap());
        let lat = i32::from_be_bytes(value[4..8].try_into().unwrap());
        NodeLonLatValue::new(lon, lat)
    }
}

pub struct NodeIdToLonLatTDC;

impl TempDataCodec for NodeIdToLonLatTDC {
    type Key = OsmIdKey;

    type Value = NodeLonLatValue;

    const NAME: &'static str = "NODE_ID_TO_LON_LAT";
}

pub struct NodeIdToIdxTDC;

impl TempDataCodec for NodeIdToIdxTDC {
    type Key = OsmIdKey;

    type Value = OsmIdxValue;

    const NAME: &'static str = "NODE_ID_TO_IDX";
}

#[cfg(test)]
mod tests {
    use super::NodeValue;
    use crate::processing::Value;

    #[test]
    fn node_value_roundtrips_with_coords_and_tags() {
        let v = NodeValue::new(-180_000_000, 90_000_000, vec![(1, 2), (3, 4)]);
        let back = NodeValue::from(v.serialize().into_boxed_slice());
        assert_eq!(back.lon, -180_000_000);
        assert_eq!(back.lat, 90_000_000);
        assert_eq!(back.refs, vec![(1, 2), (3, 4)]);
    }

    #[test]
    fn node_value_roundtrips_without_tags() {
        let back = NodeValue::from(NodeValue::new(7, -7, vec![]).serialize().into_boxed_slice());
        assert_eq!((back.lon, back.lat), (7, -7));
        assert!(back.refs.is_empty());
    }
}
