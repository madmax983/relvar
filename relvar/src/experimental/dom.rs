//! Relational DOM Engine
//!
//! This module models a Document Object Model (DOM) using purely relational
//! algebra. Nodes, parent-child edges, and attributes are represented as relations.
//! Complex CSS queries (e.g., descendant selectors) are evaluated declaratively
//! using transitive closure (`tclose`) and joins.

use relvar_core::{error::DatabaseError, values::Relation};

/// A Relational Document Object Model (DOM).
/// # Examples
///
/// ```
/// use relvar_core::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::dom::RelationalDom;
///
/// let node_type = TupleType::new()
///     .with_attribute("node_id", ScalarType::Int)
///     .with_attribute("tag", ScalarType::String);
/// let nodes = Relation::new(RelationType::new(node_type));
///
/// let edge_type = TupleType::new()
///     .with_attribute("parent_id", ScalarType::Int)
///     .with_attribute("child_id", ScalarType::Int);
/// let edges = Relation::new(RelationType::new(edge_type));
///
/// let attr_type = TupleType::new()
///     .with_attribute("node_id", ScalarType::Int)
///     .with_attribute("attr_name", ScalarType::String)
///     .with_attribute("attr_value", ScalarType::String);
/// let attributes = Relation::new(RelationType::new(attr_type));
///
/// let dom = RelationalDom::new(nodes, edges, attributes);
/// ```
pub struct RelationalDom {
    /// The DOM nodes. Schema: (node_id: Int, tag: String)
    pub nodes: Relation,
    /// The parent-child relationships. Schema: (parent_id: Int, child_id: Int)
    pub edges: Relation,
    /// Node attributes (e.g., class, id). Schema: (node_id: Int, attr_name: String, attr_value: String)
    pub attributes: Relation,
}

impl RelationalDom {
    /// Creates a new Relational DOM.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::dom::RelationalDom;
    ///
    /// let node_type = TupleType::new()
    ///     .with_attribute("node_id", ScalarType::Int)
    ///     .with_attribute("tag", ScalarType::String);
    /// let nodes = Relation::new(RelationType::new(node_type));
    ///
    /// let edge_type = TupleType::new()
    ///     .with_attribute("parent_id", ScalarType::Int)
    ///     .with_attribute("child_id", ScalarType::Int);
    /// let edges = Relation::new(RelationType::new(edge_type));
    ///
    /// let attr_type = TupleType::new()
    ///     .with_attribute("node_id", ScalarType::Int)
    ///     .with_attribute("attr_name", ScalarType::String)
    ///     .with_attribute("attr_value", ScalarType::String);
    /// let attributes = Relation::new(RelationType::new(attr_type));
    ///
    /// let dom = RelationalDom::new(nodes, edges, attributes);
    /// ```
    pub fn new(nodes: Relation, edges: Relation, attributes: Relation) -> Self {
        Self {
            nodes,
            edges,
            attributes,
        }
    }

    /// Finds all nodes with a specific class name.
    /// Evaluated by restricting the attributes relation to `class` and the target value,
    /// then projecting the `node_id`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::dom::RelationalDom;
    ///
    /// let node_type = TupleType::new()
    ///     .with_attribute("node_id", ScalarType::Int)
    ///     .with_attribute("tag", ScalarType::String);
    /// let mut nodes = Relation::new(RelationType::new(node_type));
    /// nodes.insert(tuple! { node_id: 1i64, tag: "div" }).unwrap();
    ///
    /// let edge_type = TupleType::new()
    ///     .with_attribute("parent_id", ScalarType::Int)
    ///     .with_attribute("child_id", ScalarType::Int);
    /// let edges = Relation::new(RelationType::new(edge_type));
    ///
    /// let attr_type = TupleType::new()
    ///     .with_attribute("node_id", ScalarType::Int)
    ///     .with_attribute("attr_name", ScalarType::String)
    ///     .with_attribute("attr_value", ScalarType::String);
    /// let mut attributes = Relation::new(RelationType::new(attr_type));
    /// attributes.insert(tuple! { node_id: 1i64, attr_name: "class", attr_value: "container" }).unwrap();
    ///
    /// let dom = RelationalDom::new(nodes, edges, attributes);
    ///
    /// let result = dom.query_class("container").unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    pub fn query_class(&self, class_name: &str) -> Result<Relation, DatabaseError> {
        let class_name_str = class_name.to_string();
        let res = self
            .attributes
            .restrict(move |t| {
                let name = t.get_typed::<String>("attr_name").unwrap();
                let val = t.get_typed::<String>("attr_value").unwrap();
                name == "class" && val == class_name_str
            })
            .project(&["node_id"]);
        Ok(res)
    }

    /// Evaluates a descendant selector (e.g., `ancestor descendant`).
    /// Given a relation of ancestor node IDs, this uses transitive closure (`tclose`)
    /// on the edges relation to find all reachable descendants, and then intersects
    /// that with the target descendant nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::dom::RelationalDom;
    ///
    /// let node_type = TupleType::new()
    ///     .with_attribute("node_id", ScalarType::Int)
    ///     .with_attribute("tag", ScalarType::String);
    /// let mut nodes = Relation::new(RelationType::new(node_type));
    /// nodes.insert(tuple! { node_id: 1i64, tag: "div" }).unwrap();
    /// nodes.insert(tuple! { node_id: 2i64, tag: "p" }).unwrap();
    ///
    /// let edge_type = TupleType::new()
    ///     .with_attribute("parent_id", ScalarType::Int)
    ///     .with_attribute("child_id", ScalarType::Int);
    /// let mut edges = Relation::new(RelationType::new(edge_type));
    /// edges.insert(tuple! { parent_id: 1i64, child_id: 2i64 }).unwrap();
    ///
    /// let attr_type = TupleType::new()
    ///     .with_attribute("node_id", ScalarType::Int)
    ///     .with_attribute("attr_name", ScalarType::String)
    ///     .with_attribute("attr_value", ScalarType::String);
    /// let mut attributes = Relation::new(RelationType::new(attr_type));
    /// attributes.insert(tuple! { node_id: 1i64, attr_name: "class", attr_value: "container" }).unwrap();
    /// attributes.insert(tuple! { node_id: 2i64, attr_name: "class", attr_value: "text" }).unwrap();
    ///
    /// let dom = RelationalDom::new(nodes, edges, attributes);
    ///
    /// let ancestors = dom.query_class("container").unwrap();
    /// let targets = dom.query_class("text").unwrap();
    ///
    /// let result = dom.query_descendant(&ancestors, &targets).unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    pub fn query_descendant(
        &self,
        ancestor_nodes: &Relation,
        target_nodes: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let all_paths = self.edges.tclose("parent_id", "child_id")?;
        let ancestors_renamed = ancestor_nodes.rename(&[("node_id", "parent_id")]);
        let valid_paths = all_paths.join(&ancestors_renamed)?;

        let descendants = valid_paths
            .project(&["child_id"])
            .rename(&[("child_id", "node_id")]);

        descendants
            .intersect(target_nodes)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn setup_dom() -> RelationalDom {
        let node_type = TupleType::new()
            .with_attribute("node_id", ScalarType::Int)
            .with_attribute("tag", ScalarType::String);
        let mut nodes = Relation::new(RelationType::new(node_type));

        let edge_type = TupleType::new()
            .with_attribute("parent_id", ScalarType::Int)
            .with_attribute("child_id", ScalarType::Int);
        let mut edges = Relation::new(RelationType::new(edge_type));

        let attr_type = TupleType::new()
            .with_attribute("node_id", ScalarType::Int)
            .with_attribute("attr_name", ScalarType::String)
            .with_attribute("attr_value", ScalarType::String);
        let mut attributes = Relation::new(RelationType::new(attr_type));

        // Tree structure:
        // 1 (div.container)
        //   2 (div.content)
        //     3 (p.text)
        //     4 (span.highlight)
        //   5 (footer)

        nodes.insert(tuple! { node_id: 1i64, tag: "div" }).unwrap();
        nodes.insert(tuple! { node_id: 2i64, tag: "div" }).unwrap();
        nodes.insert(tuple! { node_id: 3i64, tag: "p" }).unwrap();
        nodes.insert(tuple! { node_id: 4i64, tag: "span" }).unwrap();
        nodes
            .insert(tuple! { node_id: 5i64, tag: "footer" })
            .unwrap();

        edges
            .insert(tuple! { parent_id: 1i64, child_id: 2i64 })
            .unwrap();
        edges
            .insert(tuple! { parent_id: 2i64, child_id: 3i64 })
            .unwrap();
        edges
            .insert(tuple! { parent_id: 2i64, child_id: 4i64 })
            .unwrap();
        edges
            .insert(tuple! { parent_id: 1i64, child_id: 5i64 })
            .unwrap();

        attributes
            .insert(tuple! { node_id: 1i64, attr_name: "class", attr_value: "container" })
            .unwrap();
        attributes
            .insert(tuple! { node_id: 2i64, attr_name: "class", attr_value: "content" })
            .unwrap();
        attributes
            .insert(tuple! { node_id: 3i64, attr_name: "class", attr_value: "text" })
            .unwrap();
        attributes
            .insert(tuple! { node_id: 4i64, attr_name: "class", attr_value: "highlight" })
            .unwrap();

        RelationalDom::new(nodes, edges, attributes)
    }

    #[test]
    fn test_query_class() {
        let dom = setup_dom();
        let content_nodes = dom.query_class("content").unwrap();
        assert_eq!(content_nodes.cardinality(), 1);
        let t = content_nodes.tuples().next().unwrap();
        assert_eq!(t.get_typed::<i64>("node_id").unwrap(), 2);
    }

    #[test]
    fn test_query_descendant() {
        let dom = setup_dom();

        // Find .container
        let containers = dom.query_class("container").unwrap();

        // Target: .highlight
        let targets = dom.query_class("highlight").unwrap();

        // Find .container .highlight
        let descendants = dom.query_descendant(&containers, &targets).unwrap();

        assert_eq!(descendants.cardinality(), 1);
        let t = descendants.tuples().next().unwrap();
        assert_eq!(t.get_typed::<i64>("node_id").unwrap(), 4);
    }
}
