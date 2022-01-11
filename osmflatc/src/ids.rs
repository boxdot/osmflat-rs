const ID_BLOCK_SIZE: usize = 1 << 24;
const DENSE_LOOKUP_BLOCK_SIZE: usize = 1 << 4;

/// An IdBlock can either be Sparse or Dense
/// Sparse: A sorted list of ids, the position determines the index
/// Dense: A bitset of the whole range. An additional offsets lookup
///        provides fast lookup for the index by storing the sum of
///        set bits every DENSE_LOOKUP_BLOCK_SIZE * 8 bits
#[derive(Debug, Clone)]
enum IdBlock {
    Dense {
        includes: Vec<u8>,
        offsets: Vec<u32>,
    },
    Sparse(Vec<u32>),
}

impl IdBlock {
    /// Amount if ids in the block
    fn count(&self) -> u32 {
        match self {
            IdBlock::Sparse(ids) => ids.len() as u32,
            IdBlock::Dense { offsets, includes } => {
                let last_bits: u32 = includes[includes.len() - DENSE_LOOKUP_BLOCK_SIZE..]
                    .iter()
                    .map(|x| x.count_ones() as u32)
                    .sum();
                *offsets.last().unwrap() + last_bits
            }
        }
    }

    /// adds a truncated id into the current block
    fn insert(&mut self, x: u32) {
        match self {
            IdBlock::Sparse(ids) => {
                if ids.len() * 8 < ID_BLOCK_SIZE / 8 {
                    ids.push(x)
                } else {
                    let mut dense = IdBlock::Dense {
                        includes: vec![0; ID_BLOCK_SIZE / 8],
                        offsets: vec![0; ID_BLOCK_SIZE / 8 / DENSE_LOOKUP_BLOCK_SIZE],
                    };
                    for id in ids {
                        dense.insert(*id);
                    }
                    dense.insert(x);

                    *self = dense;
                }
            }
            IdBlock::Dense { includes, .. } => includes[x as usize / 8] |= 1 << (x % 8),
        }
    }

    // established lookups
    fn finalize(&mut self) {
        if let IdBlock::Dense { includes, offsets } = self {
            for block in 0..offsets.len() - 1 {
                offsets[block + 1] = includes
                    [block * DENSE_LOOKUP_BLOCK_SIZE..(block + 1) * DENSE_LOOKUP_BLOCK_SIZE]
                    .iter()
                    .map(|x| x.count_ones() as u32)
                    .sum();
            }
            for block in 0..offsets.len() - 1 {
                offsets[block + 1] += offsets[block];
            }
        }
    }

    // find the positions/index of a truncated id (if it is in the block)
    fn pos(&self, x: u32) -> Option<u32> {
        match self {
            IdBlock::Sparse(ids) => ids.binary_search(&x).ok().map(|x| x as u32),
            IdBlock::Dense { includes, offsets } => {
                if (includes[x as usize / 8] & (1 << (x % 8))) == 0 {
                    None
                } else {
                    let offset_pos = x as usize / 8 / DENSE_LOOKUP_BLOCK_SIZE;
                    let start_block = offset_pos * 8 * DENSE_LOOKUP_BLOCK_SIZE;
                    let rest = x as usize % (8 * DENSE_LOOKUP_BLOCK_SIZE);
                    let mut result = offsets[offset_pos];
                    for i in start_block..start_block + rest {
                        result += ((includes[i as usize / 8] & (1 << (i % 8))) != 0) as u32;
                    }
                    Some(result)
                }
            }
        }
    }
}

/// Maps u64 integers to a consecutive range of ids
#[derive(Debug)]
pub struct IdTable {
    // map u64 id x to u32 by storing a sorted mapping table for each value of x / 2^24
    data: Vec<(u64, IdBlock)>,
}

#[derive(Debug, Default)]
pub struct IdTableBuilder {
    // stored the same data as IdTable, but still in process of being build
    data: Vec<IdBlock>,
    last_id: Option<u64>,
    next_id: u64,
}

impl IdTableBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Inserts an Id and returns a mapped index
    pub fn insert(&mut self, x: u64) -> u64 {
        if let Some(last_id) = self.last_id {
            assert!(last_id < x, "Ids are expected to be sorted");
        }
        self.last_id = Some(x);
        let id_set = (x >> 24) as usize;
        if self.data.len() <= id_set {
            self.data.resize(id_set + 1, IdBlock::Sparse(Vec::new()));
        }
        self.data[id_set].insert((x % (1u64 << 24)) as u32);
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    pub fn build(mut self) -> IdTable {
        for ids in &mut self.data {
            ids.finalize();
        }
        let result = self
            .data
            .into_iter()
            .scan(0, |state, ids| {
                let offset = *state;
                *state += ids.count() as u64;
                Some((offset, ids))
            })
            .collect();
        IdTable { data: result }
    }
}

impl IdTable {
    pub fn get(&self, x: u64) -> Option<u64> {
        let id_set = (x >> 24) as usize;
        if id_set > self.data.len() {
            return None;
        }
        self.data[id_set]
            .1
            .pos((x % (1u64 << 24)) as u32)
            .map(|pos| self.data[id_set].0 + pos as u64)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mapping_of_small_ints() {
        let mut builder = IdTableBuilder::new();
        let mut data = [9, 8, 7, 4, 3, 10, 13];
        data.sort_unstable();
        for x in data.iter() {
            builder.insert(*x);
        }

        let lookup = builder.build();
        for (pos, x) in data.iter().enumerate() {
            let res = lookup.get(*x);
            assert_eq!(res, Some(pos as u64));
        }

        for x in [0, 1, 2, 5, 6, 11, 12, 14].iter() {
            let res = lookup.get(*x);
            assert_eq!(res, None);
        }
    }

    #[test]
    fn test_mapping_of_large_ints() {
        let mut builder = IdTableBuilder::new();
        let mut data = [2, 1, 1_u64 << 33, 1_u64 << 34];
        data.sort_unstable();
        for x in data.iter() {
            builder.insert(*x);
        }

        let lookup = builder.build();
        for (pos, x) in data.iter().enumerate() {
            let res = lookup.get(*x);
            assert_eq!(res, Some(pos as u64));
        }

        for x in [0, 3, (1_u64 << 33) + 1, (1_u64 << 34) + 1, 1_u64 << 35].iter() {
            let res = lookup.get(*x);
            assert_eq!(res, None);
        }
    }

    #[test]
    fn test_large_indices() {
        let mut builder = IdTableBuilder::new();
        let mut data = [2, 1, 1_u64 << 33, 1_u64 << 34];
        data.sort_unstable();
        for x in data.iter() {
            builder.insert(*x);
        }

        let lookup = builder.build();
        for (pos, x) in data.iter().enumerate() {
            let res = lookup.get(*x);
            assert_eq!(res, Some(pos as u64));
        }

        for x in [0, 3, (1_u64 << 33) + 1, (1_u64 << 34) + 1, 1_u64 << 35].iter() {
            let res = lookup.get(*x);
            assert_eq!(res, None);
        }
    }

    #[test]
    fn test_dense() {
        let mut builder = IdTableBuilder::new();
        let mut data = Vec::new();
        for i in 0..ID_BLOCK_SIZE {
            data.push(i as u64 * 3 + (1_u64 << 34));
        }
        data.sort_unstable();
        for x in data.iter() {
            builder.insert(*x);
        }

        let lookup = builder.build();
        for i in 0..ID_BLOCK_SIZE * 3 {
            let res = lookup.get(i as u64 + (1_u64 << 34));
            if i % 3 == 0 {
                assert_eq!(Some(i as u64 / 3), res);
            } else {
                assert_eq!(None, res);
            }
        }
    }
}
