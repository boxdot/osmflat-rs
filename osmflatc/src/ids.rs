use rayon::prelude::*;

/// Maps u64 integers to a consecutive range of ids
#[derive(Debug)]
pub struct IdTable {
    // map u64 id x to u32 by storing a sorted mapping table for each value of x / 2^24
    // each mapping entry (u64) represents (u24) id set (x % 2^24), and mapped id (u40)
    data: Vec<Vec<u64>>,
}

#[derive(Debug, Default)]
pub struct IdTableBuilder {
    // stored the same data as IdTable, but not yet sorted
    data: Vec<Vec<u64>>,
    next_id: u64,
}

// pack index compactly in 8 bytes: supports 1 trillion indices
fn pack_index(x: (u32, u64)) -> u64 {
    assert!(x.0 < (1_u32 << 24));
    assert!(x.1 < (1_u64 << 40));
    x.1 | (u64::from(x.0) << 40)
}

fn unpack_packed_index(x: u64) -> (u32, u64) {
    ((x >> 40) as u32, x % (1_u64 << 40))
}

impl IdTableBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Inserts an Id and returns a mapped index
    pub fn insert(&mut self, x: u64) -> u64 {
        let id_set = (x >> 24) as usize;
        if self.data.len() <= id_set {
            self.data.resize(id_set + 1, Vec::new());
        }
        self.data[id_set].push(pack_index(((x % (1u64 << 24)) as u32, self.next_id)));
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    pub fn build(mut self) -> IdTable {
        self.data.par_iter_mut().for_each(|x| x.par_sort_unstable());

        IdTable { data: self.data }
    }
}

impl IdTable {
    pub fn get(&self, x: u64) -> Option<u64> {
        let id_set = (x >> 24) as usize;
        if id_set > self.data.len() {
            return None;
        }
        self.data[id_set]
            .binary_search_by_key(&((x % (1u64 << 24)) as u32), |item| {
                unpack_packed_index(*item).0
            })
            .ok()
            .map(|pos| unpack_packed_index(self.data[id_set][pos]).1)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mapping_of_small_ints() {
        let mut builder = IdTableBuilder::new();
        let data = [9, 8, 7, 4, 3, 10, 13];
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
        let data = [2, 1, 1_u64 << 33, 1_u64 << 34];
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
        builder.next_id += 1u64 << 33;
        let data = [2, 1, 1_u64 << 33, 1_u64 << 34];
        for x in data.iter() {
            builder.insert(*x);
        }

        let lookup = builder.build();
        for (pos, x) in data.iter().enumerate() {
            let res = lookup.get(*x);
            assert_eq!(res, Some((pos as u64) + (1u64 << 33)));
        }

        for x in [0, 3, (1_u64 << 33) + 1, (1_u64 << 34) + 1, 1_u64 << 35].iter() {
            let res = lookup.get(*x);
            assert_eq!(res, None);
        }
    }
}
