use std::fmt;
use std::ops::AddAssign;

use ahash::AHashSet;

#[derive(Debug, Default)]
pub struct Stats {
    pub num_nodes: usize,
    pub num_ways: usize,
    pub num_relations: usize,
}

impl AddAssign for Stats {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.num_nodes += other.num_nodes;
        self.num_ways += other.num_ways;
        self.num_relations += other.num_relations;
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            r#"Converted:
  nodes:        {}
  ways:         {}
  relations:    {}"#,
            self.num_nodes, self.num_ways, self.num_relations,
        )
    }
}

/// Referenced objects that are not present in the archive, using the same
/// categories as `osmium check-refs -r`. Each count is the number of *distinct*
/// missing object ids (matching osmium's `-i` listing). Note osmium's summary
/// line instead counts occurrences, so it can be slightly higher when one
/// missing object is referenced by several parents.
#[derive(Debug, Default)]
pub struct MissingRefs {
    pub nodes_in_ways: AHashSet<i64>,
    pub nodes_in_relations: AHashSet<i64>,
    pub ways_in_relations: AHashSet<i64>,
    pub relations_in_relations: AHashSet<i64>,
}

impl fmt::Display for MissingRefs {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            r#"Missing references:
  nodes in ways:           {}
  nodes in relations:      {}
  ways in relations:       {}
  relations in relations:  {}"#,
            self.nodes_in_ways.len(),
            self.nodes_in_relations.len(),
            self.ways_in_relations.len(),
            self.relations_in_relations.len(),
        )
    }
}
