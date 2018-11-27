/// Maps u64 integers to a consecutive range of ids
#[derive(Debug)]
pub struct IdTable {
    // map u64 id x to u32 by storing a sorted mapping table for each value of x / 2^32
    data: Vec<Vec<(u32, u32)>>,
    num_ids: u32,
}

#[derive(Debug, Default)]
pub struct IdTableBuilder {
    // stored the same data as IdTable, but not yet sorted
    data: Vec<Vec<(u32, u32)>>,
    next_id: u32,
}

impl IdTableBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Inserts an Id and returns a mapped index
    pub fn insert(&mut self, x: u64) -> u32 {
        let id_set = (x >> 32) as usize;
        if self.data.len() <= id_set {
            self.data.resize(id_set + 1, Vec::new());
        }
        self.data[id_set].push((x as u32, self.next_id));
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    /// Skips a few ids (e.g. to reserve them for other uses)
    pub fn skip(&mut self, count: u32) {
        self.next_id += count;
    }

    pub fn build(mut self) -> IdTable {
        for mut set in &mut self.data {
            set.sort();
        }

        IdTable {
            data: self.data,
            num_ids: self.next_id,
        }
    }
}

impl IdTable {
    pub fn get(&self, x: u64) -> Option<u32> {
        let id_set = (x >> 32) as usize;
        if id_set > self.data.len() {
            return None;
        }
        self.data[id_set]
            .binary_search_by_key(&(x as u32), |item| item.0)
            .ok()
            .map(|pos| self.data[id_set][pos].1)
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
            assert_eq!(res, Some(pos as u32));
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
            assert_eq!(res, Some(pos as u32));
        }

        for x in [0, 3, (1_u64 << 33) + 1, (1_u64 << 34) + 1, 1_u64 << 35].iter() {
            let res = lookup.get(*x);
            assert_eq!(res, None);
        }
    }
}
