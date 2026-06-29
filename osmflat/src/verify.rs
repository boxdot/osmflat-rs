//! Internal-consistency verification for an [`Osm`] archive.
//!
//! [`verify`] runs structural and ordering invariant checks over an opened
//! archive and returns a [`Report`]. It needs only the archive itself (no
//! source PBF) and reuses the exact spatial-index and id-lookup functions the
//! queries use, so a clean report means the queries' preconditions hold.
//!
//! Checks performed:
//! 1. **Referential integrity** — every *resolved* index is in bounds
//!    (`None`/`INVALID_IDX` is a legal "missing at extract boundary" state and
//!    is counted, not flagged).
//! 2. **`@range` tiling** — tag/ref ranges are contiguous across each entity
//!    vector, chain across the three entity types, and end at the referenced
//!    vector's length.
//! 3. **Implicit binding** — `relation_members` outer length equals `relations`
//!    length, and each member's referenced index is in bounds.
//! 4. **Spatial ordering** — the per-entity sort key is non-decreasing, i.e.
//!    the precondition the binary-search queries rely on.
//! 5. **Id permutation** — `ids.*_by_id` is a strictly-sorted bijection of `[0,
//!    len)` and the reverse lookup round-trips.

use crate::{
    node_curve, node_idx_by_id, relation_idx_by_id, spatial_index_node, spatial_index_relation,
    spatial_index_way, way_curve, way_idx_by_id, Id, IdxRef, Osm, RelationMembersRef,
};

/// Maximum number of example violations retained (the `total` count is exact).
const MAX_EXAMPLES: usize = 100;

/// Number of evenly-spaced ids round-tripped through the reverse lookup per
/// vector (the full bijection/sort check is exhaustive; this just exercises the
/// public lookup code against the data).
const ROUND_TRIP_SAMPLES: usize = 1000;

/// A single invariant violation. Fields name the offending resource and
/// location; see each variant's doc for the invariant that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Violation {
    /// A resolved index points outside its target vector.
    IndexOutOfBounds {
        resource: &'static str,
        at: usize,
        value: u64,
        len: usize,
    },
    /// A `@range` start does not meet the previous element's end (the backing
    /// `*_first_idx` is not monotonically non-decreasing / not contiguous).
    RangeNotContiguous {
        resource: &'static str,
        at: usize,
        expected_start: u64,
        found_start: u64,
    },
    /// The final `@range` end does not equal the referenced vector's length.
    RangeTailMismatch {
        resource: &'static str,
        end: u64,
        len: usize,
    },
    /// `relation_members` outer length disagrees with `relations`.
    ImplicitBindingLen { members: usize, relations: usize },
    /// The spatial sort key decreased between consecutive entities.
    SpatialOrderRegression {
        resource: &'static str,
        at: usize,
        prev: u64,
        curr: u64,
    },
    /// An `ids.X` length disagrees with the entity count.
    IdLenMismatch {
        resource: &'static str,
        ids: usize,
        entities: usize,
    },
    /// The id permutation is not a strictly-sorted bijection of `[0, len)`.
    BadPermutation {
        resource: &'static str,
        at: usize,
        detail: &'static str,
    },
    /// `id -> index` reverse lookup did not return the expected index.
    IdRoundTrip { resource: &'static str, id: u64 },
    /// A stringtable index is out of bounds or not at a string start.
    BadStringIndex { resource: &'static str, idx: u64 },
}

/// What was scanned, for context alongside any violations.
#[derive(Debug, Default, Clone)]
#[allow(missing_docs)]
pub struct VerifyStats {
    pub nodes: usize,
    pub ways: usize,
    pub relations: usize,
    pub members: usize,
    /// Refs/members that resolve to `None` — expected for entities whose target
    /// lies outside the extract; counted, never a violation.
    pub missing_refs: u64,
    pub ids_present: bool,
    pub reverse_index_present: bool,
}

/// Outcome of [`verify`].
#[derive(Debug, Clone)]
pub struct Report {
    /// Up to [`MAX_EXAMPLES`] example violations.
    pub violations: Vec<Violation>,
    /// Exact total number of violations (may exceed `violations.len()`).
    pub total: u64,
    /// Counts of what was scanned.
    pub stats: VerifyStats,
}

impl Report {
    /// `true` when no invariant was violated.
    pub fn is_clean(&self) -> bool {
        self.total == 0
    }
}

struct Collector {
    violations: Vec<Violation>,
    total: u64,
}

impl Collector {
    fn push(&mut self, v: Violation) {
        self.total += 1;
        if self.violations.len() < MAX_EXAMPLES {
            self.violations.push(v);
        }
    }
}

/// Verify the internal consistency of `archive`. See the module docs for the
/// list of checks.
pub fn verify(archive: &Osm) -> Report {
    let mut c = Collector {
        violations: Vec::new(),
        total: 0,
    };
    let mut stats = VerifyStats::default();

    let nodes = archive.nodes();
    let ways = archive.ways();
    let relations = archive.relations();
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let nodes_index = archive.nodes_index();
    let strings = archive.stringtable().as_bytes();

    let (n_nodes, n_ways, n_relations) = (nodes.len(), ways.len(), relations.len());
    let (n_tags, n_tags_index, n_strings) = (tags.len(), tags_index.len(), strings.len());
    stats.nodes = n_nodes;
    stats.ways = n_ways;
    stats.relations = n_relations;

    let string_ok = |idx: u64| -> bool {
        let i = idx as usize;
        i < n_strings && (i == 0 || strings[i - 1] == 0)
    };

    // ---- #1 referential integrity ----
    for (i, ti) in tags_index.iter().enumerate() {
        if ti.value() as usize >= n_tags {
            c.push(Violation::IndexOutOfBounds {
                resource: "tags_index",
                at: i,
                value: ti.value(),
                len: n_tags,
            });
        }
    }
    for (i, ni) in nodes_index.iter().enumerate() {
        match ni.value() {
            Some(v) if v as usize >= n_nodes => c.push(Violation::IndexOutOfBounds {
                resource: "nodes_index",
                at: i,
                value: v,
                len: n_nodes,
            }),
            None => stats.missing_refs += 1,
            _ => {}
        }
    }
    for tag in tags.iter() {
        if !string_ok(tag.key_idx()) {
            c.push(Violation::BadStringIndex {
                resource: "tag.key_idx",
                idx: tag.key_idx(),
            });
        }
        if !string_ok(tag.value_idx()) {
            c.push(Violation::BadStringIndex {
                resource: "tag.value_idx",
                idx: tag.value_idx(),
            });
        }
    }
    let h = archive.header();
    for (res, idx) in [
        ("header.writingprogram_idx", h.writingprogram_idx()),
        ("header.source_idx", h.source_idx()),
        (
            "header.replication_base_url_idx",
            h.replication_base_url_idx(),
        ),
    ] {
        if idx as usize >= n_strings {
            c.push(Violation::BadStringIndex { resource: res, idx });
        }
    }

    // ---- #3 implicit binding + member bounds ----
    let members = archive.relation_members();
    if members.len() != n_relations {
        c.push(Violation::ImplicitBindingLen {
            members: members.len(),
            relations: n_relations,
        });
    }
    for r in 0..members.len().min(n_relations) {
        for m in members.at(r) {
            stats.members += 1;
            let (resource, idx, role, role_res, len) = match m {
                RelationMembersRef::NodeMember(m) => (
                    "node_member.node_idx",
                    m.node_idx(),
                    m.role_idx(),
                    "node_member.role_idx",
                    n_nodes,
                ),
                RelationMembersRef::WayMember(m) => (
                    "way_member.way_idx",
                    m.way_idx(),
                    m.role_idx(),
                    "way_member.role_idx",
                    n_ways,
                ),
                RelationMembersRef::RelationMember(m) => (
                    "relation_member.relation_idx",
                    m.relation_idx(),
                    m.role_idx(),
                    "relation_member.role_idx",
                    n_relations,
                ),
            };
            match idx {
                Some(v) if v as usize >= len => c.push(Violation::IndexOutOfBounds {
                    resource,
                    at: r,
                    value: v,
                    len,
                }),
                None => stats.missing_refs += 1,
                _ => {}
            }
            if role as usize >= n_strings {
                c.push(Violation::BadStringIndex {
                    resource: role_res,
                    idx: role,
                });
            }
        }
    }

    // ---- #2 @range tiling: tags_index chains nodes -> ways -> relations ----
    let node_tag_end = check_ranges(
        &mut c,
        "nodes.tags",
        nodes.iter().map(|n| n.tags()),
        0,
        n_tags_index,
    );
    let way_tag_end = check_ranges(
        &mut c,
        "ways.tags",
        ways.iter().map(|w| w.tags()),
        node_tag_end,
        n_tags_index,
    );
    let rel_tag_end = check_ranges(
        &mut c,
        "relations.tags",
        relations.iter().map(|r| r.tags()),
        way_tag_end,
        n_tags_index,
    );
    if rel_tag_end != n_tags_index as u64 {
        c.push(Violation::RangeTailMismatch {
            resource: "tags_index",
            end: rel_tag_end,
            len: n_tags_index,
        });
    }
    let ref_end = check_ranges(
        &mut c,
        "ways.refs",
        ways.iter().map(|w| w.refs()),
        0,
        nodes_index.len(),
    );
    if ref_end != nodes_index.len() as u64 {
        c.push(Violation::RangeTailMismatch {
            resource: "nodes_index",
            end: ref_end,
            len: nodes_index.len(),
        });
    }

    // ---- #4 spatial ordering (binary-search precondition) ----
    {
        let curve = node_curve();
        let cs = h.coord_scale();
        let mut prev = 0u64;
        for (i, n) in nodes.iter().enumerate() {
            let k = spatial_index_node(&curve, n, cs);
            if k < prev {
                c.push(Violation::SpatialOrderRegression {
                    resource: "nodes",
                    at: i,
                    prev,
                    curr: k,
                });
            }
            prev = k;
        }
    }
    {
        let curve = way_curve();
        let cs = h.coord_scale() as f64;
        let mut prev = 0u64;
        for i in 0..n_ways {
            let k = spatial_index_way(archive, &curve, i, cs);
            if k < prev {
                c.push(Violation::SpatialOrderRegression {
                    resource: "ways",
                    at: i,
                    prev,
                    curr: k,
                });
            }
            prev = k;
        }
    }
    {
        let curve = way_curve();
        let cs = h.coord_scale() as f64;
        let mut prev = 0u64;
        for (i, r) in relations.iter().enumerate() {
            let k = spatial_index_relation(&curve, r, cs);
            if k < prev {
                c.push(Violation::SpatialOrderRegression {
                    resource: "relations",
                    at: i,
                    prev,
                    curr: k,
                });
            }
            prev = k;
        }
    }

    // ---- #5 id permutation + round-trip ----
    if let Some(ids) = archive.ids() {
        stats.ids_present = true;
        check_id_len(&mut c, "nodes", ids.nodes().len(), n_nodes);
        check_id_len(&mut c, "ways", ids.ways().len(), n_ways);
        check_id_len(&mut c, "relations", ids.relations().len(), n_relations);

        stats.reverse_index_present = ids.nodes_by_id().is_some()
            || ids.ways_by_id().is_some()
            || ids.relations_by_id().is_some();

        if let Some(perm) = ids.nodes_by_id() {
            check_permutation(&mut c, "nodes_by_id", perm, ids.nodes());
            check_round_trip(&mut c, "nodes", ids.nodes(), |id| {
                node_idx_by_id(archive, id)
            });
        }
        if let Some(perm) = ids.ways_by_id() {
            check_permutation(&mut c, "ways_by_id", perm, ids.ways());
            check_round_trip(&mut c, "ways", ids.ways(), |id| way_idx_by_id(archive, id));
        }
        if let Some(perm) = ids.relations_by_id() {
            check_permutation(&mut c, "relations_by_id", perm, ids.relations());
            check_round_trip(&mut c, "relations", ids.relations(), |id| {
                relation_idx_by_id(archive, id)
            });
        }
    }

    Report {
        violations: c.violations,
        total: c.total,
        stats,
    }
}

/// Walk an entity vector's `@range`s, asserting each starts where the previous
/// ended (`region_start` for the first), that ends are within `target_len`, and
/// returning the final end (the region boundary / sentinel value).
fn check_ranges(
    c: &mut Collector,
    resource: &'static str,
    ranges: impl Iterator<Item = std::ops::Range<u64>>,
    region_start: u64,
    target_len: usize,
) -> u64 {
    let mut expected = region_start;
    for (i, r) in ranges.enumerate() {
        if r.start != expected {
            c.push(Violation::RangeNotContiguous {
                resource,
                at: i,
                expected_start: expected,
                found_start: r.start,
            });
        }
        if r.end < r.start || r.end as usize > target_len {
            c.push(Violation::IndexOutOfBounds {
                resource,
                at: i,
                value: r.end,
                len: target_len,
            });
        }
        expected = r.end;
    }
    expected
}

fn check_id_len(c: &mut Collector, resource: &'static str, ids: usize, entities: usize) {
    if ids != entities {
        c.push(Violation::IdLenMismatch {
            resource,
            ids,
            entities,
        });
    }
}

/// `perm` must be a bijection of `[0, ids.len())` ordered so `ids[perm[k]]` is
/// strictly ascending (strict ⇒ ids are unique within the type).
fn check_permutation(c: &mut Collector, resource: &'static str, perm: &[IdxRef], ids: &[Id]) {
    if perm.len() != ids.len() {
        c.push(Violation::IdLenMismatch {
            resource,
            ids: perm.len(),
            entities: ids.len(),
        });
    }
    let mut seen = vec![false; ids.len()];
    let mut prev: Option<u64> = None;
    for (k, p) in perm.iter().enumerate() {
        let idx = p.value() as usize;
        if idx >= ids.len() {
            c.push(Violation::IndexOutOfBounds {
                resource,
                at: k,
                value: p.value(),
                len: ids.len(),
            });
            continue;
        }
        if seen[idx] {
            c.push(Violation::BadPermutation {
                resource,
                at: k,
                detail: "duplicate index",
            });
        } else {
            seen[idx] = true;
        }
        let id = ids[idx].value();
        if prev.is_some_and(|p| id <= p) {
            c.push(Violation::BadPermutation {
                resource,
                at: k,
                detail: "not strictly ascending",
            });
        }
        prev = Some(id);
    }
}

fn check_round_trip(
    c: &mut Collector,
    resource: &'static str,
    ids: &[Id],
    lookup: impl Fn(u64) -> Option<usize>,
) {
    let n = ids.len();
    if n == 0 {
        return;
    }
    let step = n.div_ceil(ROUND_TRIP_SAMPLES).max(1);
    let mut i = 0;
    while i < n {
        let id = ids[i].value();
        if lookup(id) != Some(i) {
            c.push(Violation::IdRoundTrip { resource, id });
        }
        i += step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_archive, build_node_archive_with_ids};

    #[test]
    fn clean_archive_passes() {
        let nodes = vec![(-93.0, 45.0), (2.3, 48.8), (139.7, 35.7), (-0.1, 51.5)];
        let ways = vec![vec![0, 1], vec![2, 3], vec![1, 2, 3]];
        let rels = vec![Some((-93.2, 44.9, -92.9, 45.2)), None];
        let report = verify(&build_archive(&nodes, &ways, &rels));
        assert!(report.is_clean(), "violations: {:?}", report.violations);
        assert_eq!(report.stats.nodes, 4);
        assert_eq!(report.stats.ways, 3);
        assert_eq!(report.stats.relations, 2);
    }

    #[test]
    fn archive_with_reverse_ids_passes() {
        let nodes = vec![
            (-122.4, 37.8, 42),
            (2.35, 48.85, 7),
            (139.7, 35.7, 1000),
            (-0.12, 51.5, 256),
        ];
        let report = verify(&build_node_archive_with_ids(&nodes));
        assert!(report.is_clean(), "violations: {:?}", report.violations);
        assert!(report.stats.ids_present);
        assert!(report.stats.reverse_index_present);
    }

    #[test]
    fn empty_archive_passes() {
        let report = verify(&build_archive(&[], &[], &[]));
        assert!(report.is_clean(), "violations: {:?}", report.violations);
    }
}
