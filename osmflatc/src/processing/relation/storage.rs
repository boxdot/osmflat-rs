use std::collections::BTreeSet;

/// RocksDB column family holding the encoded relations, keyed by spatial order.
pub const RELATIONS: &str = "relations";
/// RocksDB column family holding the relations' string references, in the same
/// order as [`RELATIONS`].
pub const RELATIONS_STRING_REFS: &str = "relations_string_refs";

pub fn create_relation_values(string_refs: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 * string_refs.len());
    for s in string_refs {
        out.extend(s.to_be_bytes());
    }
    out
}

pub fn break_relation_values(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks(8)
        .map(|chunk| u64::from_be_bytes(chunk[0..8].try_into().unwrap()))
        .collect()
}

#[derive(Debug, Default)]
pub struct RelationInfo {
    pub id: i64,
    pub points: Vec<(i32, i32)>,
    pub relation_ids: BTreeSet<i64>,
}

impl RelationInfo {
    pub fn is_ready(&self) -> bool {
        self.relation_ids.is_empty()
    }
}
