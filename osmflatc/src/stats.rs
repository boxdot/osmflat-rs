use std::fmt;
use std::ops::AddAssign;

#[derive(Debug, Default)]
pub struct Stats {
    pub num_nodes: usize,
    pub num_ways: usize,
    pub num_relations: usize,
    pub num_unresolved_node_ids: usize,
    pub num_unresolved_way_ids: usize,
    pub num_unresolved_rel_ids: usize,
}

impl AddAssign for Stats {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.num_nodes += other.num_nodes;
        self.num_ways += other.num_ways;
        self.num_relations += other.num_relations;
        self.num_unresolved_node_ids += other.num_unresolved_node_ids;
        self.num_unresolved_way_ids += other.num_unresolved_way_ids;
        self.num_unresolved_rel_ids += other.num_unresolved_rel_ids;
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            r#"Converted:
  nodes:        {}
  ways:         {}
  relations:    {}
Unresolved ids:
  nodes:        {}
  ways:         {}
  relations:    {}"#,
            self.num_nodes,
            self.num_ways,
            self.num_relations,
            self.num_unresolved_node_ids,
            self.num_unresolved_way_ids,
            self.num_unresolved_rel_ids
        )
    }
}
