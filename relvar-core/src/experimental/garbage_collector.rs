use crate::{
    DatabaseError,
    database::Database,
    storage_engine::InMemoryEngine,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::Relation,
};

/// A Relational Mark-and-Sweep Garbage Collector
pub struct RelationalGC {
    db: Database<InMemoryEngine>,
}

impl RelationalGC {
    /// Creates a new RelationalGC instance
    pub fn new() -> Result<Self, DatabaseError> {
        let mut db = Database::new(InMemoryEngine::new());

        let node_type =
            RelationType::new(TupleType::new().with_attribute("node_id", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_id", ScalarType::Int)
                .with_attribute("to_id", ScalarType::Int),
        );

        db.create_relvar("roots", node_type.clone())?;
        db.create_relvar("heap", node_type)?;
        db.create_relvar("references", ref_type)?;

        Ok(Self { db })
    }

    /// Adds a node to the heap
    pub fn add_node(&mut self, id: i64) -> Result<(), DatabaseError> {
        self.db.insert("heap", tuple! { node_id: id })
    }

    /// Adds a node as a root
    pub fn add_root(&mut self, id: i64) -> Result<(), DatabaseError> {
        self.db.insert("roots", tuple! { node_id: id })?;
        self.add_node(id) // Roots are also in the heap
    }

    /// Adds a reference from one node to another
    pub fn add_reference(&mut self, from: i64, to: i64) -> Result<(), DatabaseError> {
        self.db
            .insert("references", tuple! { from_id: from, to_id: to })
    }

    /// Identifies and returns garbage nodes
    pub fn collect_garbage(&mut self) -> Result<Relation, DatabaseError> {
        let roots = self.db.query("roots")?;
        let heap = self.db.query("heap")?;
        let references = self.db.query("references")?;

        // Fast path if references is empty
        if references.cardinality() == 0 {
            return heap
                .difference(&roots)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()));
        }

        let closure = references.tclose("from_id", "to_id")?;

        // Find reachable nodes from roots
        let roots_as_from = roots.rename(&[("node_id", "from_id")]);
        let reached = closure
            .join(&roots_as_from)?
            .project(&["to_id"])
            .rename(&[("to_id", "node_id")]);

        let all_reachable = roots
            .union(&reached)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let garbage = heap
            .difference(&all_reachable)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(garbage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_basic() {
        let mut gc = RelationalGC::new().unwrap();

        // Setup nodes: 1 (root), 2 (reachable), 3 (garbage), 4 (reachable from 2), 5 (cycle with 3)
        gc.add_root(1).unwrap();
        gc.add_node(2).unwrap();
        gc.add_node(3).unwrap();
        gc.add_node(4).unwrap();
        gc.add_node(5).unwrap();

        gc.add_reference(1, 2).unwrap(); // 1 -> 2
        gc.add_reference(2, 4).unwrap(); // 2 -> 4
        gc.add_reference(3, 5).unwrap(); // 3 -> 5
        gc.add_reference(5, 3).unwrap(); // 5 -> 3 (cycle of garbage)

        let garbage = gc.collect_garbage().unwrap();

        // Expected garbage: 3 and 5
        assert_eq!(garbage.cardinality(), 2);

        // Verify contents
        let mut ids: Vec<i64> = garbage
            .tuples()
            .map(|t| t.get_typed::<i64>("node_id").unwrap())
            .collect();
        ids.sort();

        assert_eq!(ids, vec![3, 5]);
    }
}
