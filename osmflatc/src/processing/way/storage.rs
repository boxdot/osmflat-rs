use crate::processing::{
    storage::{OsmIdKey, OsmIdxValue, OsmKey},
    TempDataCodec, Value,
};

pub struct WayValue {
    pub node_refs: Vec<i64>,
    pub key_vals: Vec<(u64, u64)>,
}

impl WayValue {
    pub fn new(node_refs: Vec<i64>, key_vals: Vec<(u64, u64)>) -> Self {
        Self {
            node_refs,
            key_vals,
        }
    }
}

impl From<Box<[u8]>> for WayValue {
    fn from(bytes: Box<[u8]>) -> Self {
        let num = u64::from_be_bytes(bytes[..8].try_into().unwrap()) as usize;
        let node_refs = bytes[8..]
            .chunks(8)
            .take(num)
            .map(|v| i64::from_be_bytes(v[0..8].try_into().unwrap()))
            .collect();

        let key_vals = bytes[(num * 8 + 8)..]
            .chunks(16)
            .map(|c| {
                (
                    u64::from_be_bytes(c[0..8].try_into().unwrap()),
                    u64::from_be_bytes(c[8..16].try_into().unwrap()),
                )
            })
            .collect();

        WayValue {
            node_refs,
            key_vals,
        }
    }
}

impl Value for WayValue {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 * self.node_refs.len() + 16 * self.key_vals.len() + 8);

        let num = self.node_refs.len() as u64;

        out.extend(num.to_be_bytes());

        for r in &self.node_refs {
            out.extend(r.to_be_bytes());
        }

        for (k, v) in &self.key_vals {
            out.extend(k.to_be_bytes());
            out.extend(v.to_be_bytes());
        }

        out
    }
}

pub struct WayTDC;

impl TempDataCodec for WayTDC {
    type Key = OsmKey;

    type Value = WayValue;

    const NAME: &'static str = "WAYS";
}

pub struct WayIdToIdxTDC;

impl TempDataCodec for WayIdToIdxTDC {
    type Key = OsmIdKey;

    type Value = OsmIdxValue;

    const NAME: &'static str = "WAY_ID_TO_IDX";
}

pub struct WayMbbValue {
    pub mbb: Vec<i32>,
}

impl WayMbbValue {
    pub fn new(mbb: Vec<i32>) -> Self {
        WayMbbValue { mbb }
    }
}

impl Value for WayMbbValue {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 * self.mbb.len());
        for m in &self.mbb {
            out.extend(m.to_be_bytes());
        }
        out
    }
}

impl From<Box<[u8]>> for WayMbbValue {
    fn from(bytes: Box<[u8]>) -> Self {
        WayMbbValue {
            mbb: bytes
                .chunks(4)
                .map(|chunk| i32::from_be_bytes(chunk[0..4].try_into().unwrap()))
                .collect(),
        }
    }
}

pub struct WayIdToMbbTDC;

impl TempDataCodec for WayIdToMbbTDC {
    const NAME: &'static str = "WAY_ID_TO_MBB";

    type Key = OsmIdKey;
    type Value = WayMbbValue;
}
