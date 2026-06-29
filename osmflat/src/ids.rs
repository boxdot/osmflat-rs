//! Lookups between archive indices and original OSM ids.
//!
//! The archive stores entities in spatial-curve order, so an entity's array
//! index is unrelated to its OSM id. Two optional structures in the [`Ids`]
//! sub-archive bridge the two:
//!
//! * the positional `ids.{nodes,ways,relations}` vectors give **index -> id**
//!   in `O(1)` (written with `osmflatc --ids`), and
//! * the `ids.{nodes,ways,relations}_by_id` permutations give **id -> index**
//!   in `O(log n)` via binary search (written with `osmflatc --reverse-ids`).
//!
//! The reverse permutation stores only the target index (`u40`); the binary
//! search indirects through the positional id vector, so reverse lookups
//! require the forward `--ids` data as well (which `--reverse-ids` implies).
//!
//! [`Ids`]: crate::Ids

use crate::{Id, IdxRef, Osm};

/// OSM id of the node at `idx`, or `None` if the `ids` sub-archive was not
/// written or `idx` is out of range (e.g. the trailing sentinel node).
#[inline]
pub fn node_id(archive: &Osm, idx: usize) -> Option<u64> {
    Some(archive.ids()?.nodes().get(idx)?.value())
}

/// OSM id of the way at `idx`. See [`node_id`].
#[inline]
pub fn way_id(archive: &Osm, idx: usize) -> Option<u64> {
    Some(archive.ids()?.ways().get(idx)?.value())
}

/// OSM id of the relation at `idx`. See [`node_id`].
#[inline]
pub fn relation_id(archive: &Osm, idx: usize) -> Option<u64> {
    Some(archive.ids()?.relations().get(idx)?.value())
}

/// Index into `nodes` of the node with OSM id `id`, or `None` if no such node
/// is in the archive (or the reverse index was not written with
/// `--reverse-ids`).
#[inline]
pub fn node_idx_by_id(archive: &Osm, id: u64) -> Option<usize> {
    let ids = archive.ids()?;
    reverse_lookup(ids.nodes_by_id()?, ids.nodes(), id)
}

/// Index into `ways` of the way with OSM id `id`. See [`node_idx_by_id`].
#[inline]
pub fn way_idx_by_id(archive: &Osm, id: u64) -> Option<usize> {
    let ids = archive.ids()?;
    reverse_lookup(ids.ways_by_id()?, ids.ways(), id)
}

/// Index into `relations` of the relation with OSM id `id`.
/// See [`node_idx_by_id`].
#[inline]
pub fn relation_idx_by_id(archive: &Osm, id: u64) -> Option<usize> {
    let ids = archive.ids()?;
    reverse_lookup(ids.relations_by_id()?, ids.relations(), id)
}

/// Binary-search `perm` -- a permutation of indices into `fwd` ordered so that
/// `fwd[perm[k].value]` is ascending by id -- for `id`, returning the index
/// into `fwd` (i.e. into the entity vector) on an exact match.
#[inline]
fn reverse_lookup(perm: &[IdxRef], fwd: &[Id], id: u64) -> Option<usize> {
    // First k whose entity id is >= the target.
    let k = perm.partition_point(|p| fwd[p.value() as usize].value() < id);
    let idx = perm.get(k)?.value() as usize;
    (fwd[idx].value() == id).then_some(idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::build_node_archive_with_ids;

    #[test]
    fn node_id_and_idx_round_trip_under_spatial_reordering() {
        // Ids deliberately unsorted and unrelated to the spatial order, so a
        // correct lookup must rely on the reverse permutation, not on input
        // order. Spread out so the z-order genuinely permutes them.
        let nodes = [
            (-122.4, 37.8, 42),
            (2.35, 48.85, 7),
            (139.7, 35.7, 1000),
            (-0.12, 51.5, 256),
            (151.2, -33.9, 99),
        ];
        let archive = build_node_archive_with_ids(&nodes);

        // id -> index -> id is the identity, for every node, regardless of where
        // the spatial sort placed it.
        for &(_, _, id) in &nodes {
            let idx = node_idx_by_id(&archive, id).unwrap_or_else(|| panic!("id {id} not found"));
            assert_eq!(node_id(&archive, idx), Some(id));
        }

        // Absent ids return None rather than a wrong/over-the-edge index.
        assert_eq!(node_idx_by_id(&archive, 0), None);
        assert_eq!(node_idx_by_id(&archive, 43), None); // between 42 and 99
        assert_eq!(node_idx_by_id(&archive, u64::MAX), None);

        // Reverse index was not written for ways/relations here.
        assert_eq!(way_idx_by_id(&archive, 42), None);
        assert_eq!(relation_idx_by_id(&archive, 42), None);
    }

    #[test]
    fn missing_ids_sub_archive_yields_none() {
        // build_archive (no ids sub-archive at all) -> every lookup is None.
        let archive = crate::test_support::build_archive(&[(-93.0, 45.0)], &[], &[]);
        assert_eq!(node_idx_by_id(&archive, 1), None);
        assert_eq!(node_id(&archive, 0), None);
    }
}
