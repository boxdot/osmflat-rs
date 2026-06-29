use std::collections::BTreeMap;

use itertools::Itertools;

use super::{Key, RocksDB, RocksDBUnsync, TempDataCodec, Value};

#[derive(Debug, Default)]
pub struct MockRocksBatch {
    pub families: BTreeMap<String, BTreeMap<Vec<u8>, Vec<u8>>>,
}

impl RocksDBUnsync for MockRocksBatch {
    fn put<TDC: TempDataCodec>(&mut self, key: TDC::Key, value: TDC::Value) {
        self.families
            .entry(TDC::NAME.to_owned())
            .and_modify(|fam| {
                fam.insert(key.serialize(), value.serialize());
            })
            .or_insert_with(|| {
                let mut m = BTreeMap::default();
                m.insert(key.serialize(), value.serialize());
                m
            });
    }
}

impl RocksDB for MockRocksBatch {
    #[allow(clippy::type_complexity)]
    fn iterator<'a, TDC: TempDataCodec>(
        &'a self,
    ) -> Result<
        Box<dyn Iterator<Item = Result<(TDC::Key, TDC::Value), crate::error::OsmFlatcError>> + 'a>,
        crate::error::OsmFlatcError,
    >
    where
        <TDC as TempDataCodec>::Key: 'a,
        <TDC as TempDataCodec>::Value: 'a,
    {
        Ok(Box::new(self.families.get(TDC::NAME).into_iter().flat_map(
            |fam| {
                fam.iter()
                    .sorted_unstable_by_key(|(k, _)| k.to_vec())
                    .map(|(k, v)| {
                        Ok((
                            TDC::Key::from(k.clone().into_boxed_slice()),
                            TDC::Value::from(v.clone().into_boxed_slice()),
                        ))
                    })
            },
        )))
    }
}
