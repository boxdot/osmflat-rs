use crate::error::OsmFlatcError;
use crate::{
    add_string_table,
    osmpbf::{self, read_block, BlockIndex, PrimitiveBlock},
    pb_style,
    processing::{
        node::storage::{NodeIdToIdxTDC, NodeIdToLonLatTDC},
        storage::{OsmIdKey, OsmKey},
        way::storage::{WayIdToIdxTDC, WayIdToMbbTDC},
        Key, RocksDBSync, TempDataCodec,
    },
    stats::{MissingRefs, Stats},
    strings::StringTable,
    Error, TagSerializer,
};
use ahash::AHashMap;
use geo::{BoundingRect, MultiPoint};
use indicatif::ProgressBar;
use log::info;
use prost::Message;
use rayon::iter::{ParallelBridge, ParallelIterator};
use rocksdb::DB;
use space_time::xzorder::xz2_sfc::XZ2SFC;
use std::collections::VecDeque;
use storage::{
    break_relation_values, create_relation_values, RelationInfo, RELATIONS, RELATIONS_STRING_REFS,
};

pub(crate) mod storage;

fn build_relations_index<I>(
    data: &[u8],
    block_index: I,
    db: &DB,
) -> Result<(AHashMap<i64, RelationInfo>, Vec<RelationInfo>), Error>
where
    I: ExactSizeIterator<Item = BlockIndex> + Send + 'static,
{
    let pb = ProgressBar::new(block_index.len() as u64)
        .with_style(pb_style())
        .with_prefix("Building relations index");

    // The per-member RocksDB lookups dominate this pass and are random reads
    // against a planet-sized DB, so fan them out across all Rayon workers and
    // merge the per-block results with `try_reduce`. Order is irrelevant here:
    // `found` is keyed by id and `unresolved` is processed without regard to
    // order. `par_bridge` drives the (non-indexed) block iterator in parallel.
    let (found_list, unresolved) = block_index
        .par_bridge()
        .map(
            |idx| -> Result<(Vec<RelationInfo>, Vec<RelationInfo>), OsmFlatcError> {
                let block: osmpbf::PrimitiveBlock = read_block(data, &idx)?;
                let mut block_found = Vec::new();
                let mut block_unresolved = Vec::new();
                for group in &block.primitivegroup {
                    for pbf_relation in &group.relations {
                        let mut relation_info = RelationInfo {
                            id: pbf_relation.id,
                            ..Default::default()
                        };

                        let mut memid = 0;
                        for i in 0..pbf_relation.roles_sid.len() {
                            memid += pbf_relation.memids[i];

                            let member_type =
                                osmpbf::relation::MemberType::try_from(pbf_relation.types[i]);
                            assert!(member_type.is_ok());

                            match member_type.unwrap() {
                                osmpbf::relation::MemberType::Node => {
                                    let v = <DB as RocksDBSync>::get::<NodeIdToLonLatTDC>(
                                        db,
                                        &OsmIdKey::new(memid),
                                    )?;

                                    // Relation points are stored as (lon, lat) to match the
                                    // way-member bbox corners pushed below. Missing members are
                                    // counted later, in the emit pass, with osmium semantics.
                                    if let Some(v) = v {
                                        relation_info.points.push((v.lon, v.lat));
                                    }
                                }
                                osmpbf::relation::MemberType::Way => {
                                    let v = <DB as RocksDBSync>::get::<WayIdToMbbTDC>(
                                        db,
                                        &OsmIdKey::new(memid),
                                    )?;

                                    if let Some(mbr) = v {
                                        relation_info.points.push((mbr.mbb[0], mbr.mbb[1]));
                                        relation_info.points.push((mbr.mbb[2], mbr.mbb[3]));
                                    }
                                }
                                osmpbf::relation::MemberType::Relation => {
                                    relation_info.relation_ids.insert(memid);
                                }
                            }
                        }
                        if relation_info.is_ready() {
                            block_found.push(relation_info);
                        } else {
                            block_unresolved.push(relation_info);
                        }
                    }
                }
                pb.inc(1);
                Ok((block_found, block_unresolved))
            },
        )
        .try_reduce(
            || (Vec::new(), Vec::new()),
            |mut acc, (mut block_found, mut block_unresolved)| {
                acc.0.append(&mut block_found);
                acc.1.append(&mut block_unresolved);
                Ok(acc)
            },
        )?;
    pb.finish();

    let mut found = AHashMap::new();
    found.extend(found_list.into_iter().map(|info| (info.id, info)));

    Ok((found, unresolved))
}

fn resolve_all_relations(
    mut found: AHashMap<i64, RelationInfo>,
    unresolved: Vec<RelationInfo>,
) -> AHashMap<i64, RelationInfo> {
    let mut unresolved: VecDeque<RelationInfo> = unresolved.into();

    // Pull member geometry from sub-relations in repeated passes until a full
    // pass makes no progress (handles nesting; terminates on cycles/missing).
    loop {
        let mut progressed = false;
        let mut remaining = VecDeque::with_capacity(unresolved.len());
        while let Some(mut p) = unresolved.pop_front() {
            for rel_id in p.relation_ids.clone() {
                if let Some(f) = found.get(&rel_id) {
                    p.points.extend(&f.points);
                    p.relation_ids.remove(&rel_id);
                    progressed = true;
                }
            }
            if p.is_ready() {
                found.insert(p.id, p);
                progressed = true;
            } else {
                remaining.push_back(p);
            }
        }
        unresolved = remaining;
        if unresolved.is_empty() || !progressed {
            break;
        }
    }

    // Stop dropping: keep every relation that could not be fully resolved, with
    // whatever member geometry it accumulated. Its unresolvable relation members
    // are simply ignored (and counted as missing in the emit pass).
    for p in unresolved {
        found.entry(p.id).or_insert(p);
    }
    found
}

#[allow(clippy::too_many_arguments)]
fn serialize_relations(
    pbf_relation: &osmpbf::Relation,
    mbb: [i32; 4],
    relation_id_to_idx: &AHashMap<i64, u64>,
    db: &DB,
    relations: &mut flatdata::ExternalVector<osmflat::Relation>,
    relation_ids: &mut Option<flatdata::ExternalVector<osmflat::Id>>,
    relation_members: &mut flatdata::MultiVector<osmflat::RelationMembers>,
    string_refs: Vec<u64>,
    tags: &mut TagSerializer,
    missing: &mut MissingRefs,
) -> Result<Stats, Error> {
    let mut stats = Stats::default();

    let cf_node_id_to_idx = db.cf_handle(NodeIdToIdxTDC::NAME).unwrap();
    let cf_way_id_to_idx = db.cf_handle(WayIdToIdxTDC::NAME).unwrap();

    debug_assert_eq!(
        pbf_relation.keys.len(),
        pbf_relation.vals.len(),
        "invalid input data"
    );

    let relation = relations.grow()?;
    if let Some(ids) = relation_ids {
        ids.grow()?.set_value(pbf_relation.id as u64);
    }

    relation.set_tag_first_idx(tags.next_index());
    relation.set_min_lon(mbb[0]);
    relation.set_min_lat(mbb[1]);
    relation.set_max_lon(mbb[2]);
    relation.set_max_lat(mbb[3]);
    for i in 0..pbf_relation.keys.len() {
        tags.serialize(
            string_refs[pbf_relation.keys[i] as usize],
            string_refs[pbf_relation.vals[i] as usize],
        )?;
    }

    debug_assert!(
        pbf_relation.roles_sid.len() == pbf_relation.memids.len()
            && pbf_relation.memids.len() == pbf_relation.types.len(),
        "invalid input data"
    );

    stats.num_relations = 1;

    let mut memid = 0;
    let mut members = relation_members.grow()?;

    for i in 0..pbf_relation.roles_sid.len() {
        memid += pbf_relation.memids[i];

        let member_type = osmpbf::relation::MemberType::try_from(pbf_relation.types[i]);
        debug_assert!(member_type.is_ok());

        match member_type.unwrap() {
            osmpbf::relation::MemberType::Node => {
                let idx = db
                    .get_cf(cf_node_id_to_idx, memid.to_be_bytes())?
                    .map(|v| u64::from_be_bytes(v[0..8].try_into().unwrap()));
                if idx.is_none() {
                    missing.nodes_in_relations.insert(memid);
                }

                let member = members.add_node_member();
                member.set_node_idx(idx);
                member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
            }
            osmpbf::relation::MemberType::Way => {
                let idx = db
                    .get_cf(cf_way_id_to_idx, memid.to_be_bytes())?
                    .map(|v| u64::from_be_bytes(v[0..8].try_into().unwrap()));
                if idx.is_none() {
                    missing.ways_in_relations.insert(memid);
                }

                let member = members.add_way_member();
                member.set_way_idx(idx);
                member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
            }
            osmpbf::relation::MemberType::Relation => {
                // Resolve the referenced relation to its index in the
                // spatially-ordered relations vector. References to relations
                // not in the archive become `None` (INVALID_IDX).
                let idx = relation_id_to_idx.get(&memid).copied();
                if idx.is_none() {
                    missing.relations_in_relations.insert(memid);
                }
                let member = members.add_relation_member();
                member.set_relation_idx(idx);
                member.set_role_idx(string_refs[pbf_relation.roles_sid[i] as usize]);
            }
        }
    }
    Ok(stats)
}

/// Minimum bounding box `[min_lon, min_lat, max_lon, max_lat]` of a relation's
/// member points, scaled with `coord_scale`. Returns `None` when the relation
/// has no resolvable member geometry.
fn relation_mbb(points: &[(i32, i32)], coord_scale: i32) -> Option<[i32; 4]> {
    let cs = coord_scale as f64;
    let points: MultiPoint<f64> = points
        .iter()
        .map(|p| (p.0 as f64 / cs, p.1 as f64 / cs))
        .collect::<Vec<_>>()
        .into();
    let r = points.bounding_rect()?;
    Some([
        (r.min().x * cs) as i32,
        (r.min().y * cs) as i32,
        (r.max().x * cs) as i32,
        (r.max().y * cs) as i32,
    ])
}

/// Space-filling-curve index of a relation's bounding box. Computed from the
/// scaled `mbb` (not the raw member points) so it is identical to what the
/// query side recomputes from the stored bounding box.
fn relation_spatial_index(curve: &XZ2SFC, mbb: [i32; 4], coord_scale: i32) -> u64 {
    let cs = coord_scale as f64;
    osmflat::bbox_index(
        curve,
        mbb[0] as f64 / cs,
        mbb[1] as f64 / cs,
        mbb[2] as f64 / cs,
        mbb[3] as f64 / cs,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn serialize_relation_blocks(
    builder: &osmflat::OsmBuilder,
    db: &DB,
    mut relation_ids: Option<flatdata::ExternalVector<osmflat::Id>>,
    relation_by_id: Option<flatdata::ExternalVector<osmflat::IdxRef>>,
    blocks: Vec<BlockIndex>,
    data: &[u8],
    tags: &mut TagSerializer,
    stringtable: &mut StringTable,
    stats: &mut Stats,
    missing: &mut MissingRefs,
    coord_scale: i32,
) -> Result<(), Error> {
    // We need to build the index of relation ids first, since relations can refer
    // again to relations.
    let (found, unresolved) = build_relations_index(data, blocks.clone().into_iter(), db)?;
    let found = resolve_all_relations(found, unresolved);

    let relations_cf = db.cf_handle(RELATIONS).unwrap();
    let relations_string_refs = db.cf_handle(RELATIONS_STRING_REFS).unwrap();

    let curve = osmflat::way_curve();

    let pb = ProgressBar::new(blocks.len() as u64)
        .with_style(pb_style())
        .with_prefix("Converting relations");

    // Store every relation, keyed by its spatial index, so iterating the column
    // family yields spatial order. Relations with no resolvable member geometry
    // get the `RELATION_NO_BBOX` sentinel and a `u64::MAX` key so they sort last
    // and are never matched spatially (but are still emitted).
    for v in blocks
        .into_iter()
        .map(|idx| read_block::<PrimitiveBlock>(data, &idx))
    {
        let block = v?;

        let string_refs = add_string_table(&block.stringtable, stringtable)?;

        pb.inc(1);

        for rel in block.primitivegroup.into_iter().flat_map(|g| g.relations) {
            let id = rel.id;
            let mbb = found
                .get(&id)
                .and_then(|info| relation_mbb(&info.points, coord_scale));
            let spatial_index = match mbb {
                Some(mbb) => relation_spatial_index(&curve, mbb, coord_scale),
                None => u64::MAX,
            };
            let key = OsmKey::new(spatial_index, id).serialize();
            db.put_cf(relations_cf, &key, rel.encode_to_vec())?;
            db.put_cf(
                relations_string_refs,
                &key,
                create_relation_values(string_refs.as_slice()),
            )?;
        }
    }
    pb.finish();

    // First pass over the spatially-ordered relations: map each relation id to
    // its final index, so relation members can be resolved in the second pass
    // (a relation may reference another relation that sorts after it).
    let mut relation_id_to_idx: AHashMap<i64, u64> = AHashMap::new();
    for (idx, res) in db
        .iterator_cf(relations_cf, rocksdb::IteratorMode::Start)
        .enumerate()
    {
        let (key, _) = res?;
        relation_id_to_idx.insert(OsmKey::from(key).id, idx as u64);
    }

    let mut relations = builder.start_relations()?;
    let mut relation_members = builder.start_relation_members()?;

    let pb = ProgressBar::new(relation_id_to_idx.len() as u64)
        .with_style(pb_style())
        .with_prefix("Ordering relations");

    // Second pass: write the relations in spatial order, resolving members.
    for res in db
        .iterator_cf(relations_cf, rocksdb::IteratorMode::Start)
        .zip(db.iterator_cf(relations_string_refs, rocksdb::IteratorMode::Start))
    {
        let (key, rel) = res.0?;
        let (_, string_refs) = res.1?;

        let id = OsmKey::from(key).id;
        let relation = osmpbf::Relation::decode(rel.to_vec().as_slice())?;
        let string_refs = break_relation_values(&string_refs);

        // Relations without resolvable member geometry carry the sentinel bbox.
        let mbb = found
            .get(&id)
            .and_then(|info| relation_mbb(&info.points, coord_scale))
            .unwrap_or(osmflat::RELATION_NO_BBOX);

        *stats += serialize_relations(
            &relation,
            mbb,
            &relation_id_to_idx,
            db,
            &mut relations,
            &mut relation_ids,
            &mut relation_members,
            string_refs,
            tags,
            missing,
        )?;
        pb.inc(1);
    }

    {
        let sentinel = relations.grow()?;
        sentinel.set_tag_first_idx(tags.next_index());
    }

    relations.close()?;
    if let Some(ids) = relation_ids {
        ids.close()?;
    }
    relation_members.close()?;

    // Reverse index: unlike nodes/ways there is no id-keyed RocksDB CF for
    // relations -- `relation_id_to_idx` is an in-memory map (id -> final idx).
    // Relations are few (~tens of millions even at planet scale), so sort the
    // pairs by id and emit the indices: `ids.relations[p[k]]` then ascends by
    // id, matching the query-side binary search.
    if let Some(mut by_id) = relation_by_id {
        let mut pairs: Vec<(i64, u64)> = relation_id_to_idx.into_iter().collect();
        pairs.sort_unstable_by_key(|&(id, _)| id);
        for (_id, idx) in pairs {
            by_id.grow()?.set_value(idx);
        }
        by_id.close()?;
    }

    pb.finish();
    info!("Relations converted.");

    Ok(())
}
