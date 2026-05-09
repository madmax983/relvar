with open("relvar-core/src/query/mod.rs", "r") as f:
    content = f.read()

content = content.replace("    pub fn project<S: Into<String>, I: IntoIterator<Item = S>>(self, attributes: I) -> Self {", "    /// **Optimization**: Accepts `IntoIterator` to allow passing stack-allocated arrays\n    /// (e.g., `[\"a\", \"b\"]`) instead of requiring heap-allocated vectors (`vec![\"a\", \"b\"]`).\n    pub fn project<S: Into<String>, I: IntoIterator<Item = S>>(self, attributes: I) -> Self {")

content = content.replace("    pub fn rename<S1: Into<String>, S2: Into<String>, I: IntoIterator<Item = (S1, S2)>>(\n        self,\n        mappings: I,\n    ) -> Self {", "    /// **Optimization**: Accepts `IntoIterator` to allow passing stack-allocated arrays\n    /// (e.g., `[(\"old\", \"new\")]`) instead of requiring heap-allocated vectors.\n    pub fn rename<S1: Into<String>, S2: Into<String>, I: IntoIterator<Item = (S1, S2)>>(\n        self,\n        mappings: I,\n    ) -> Self {")

content = content.replace("    pub fn summarize<\n        S: Into<String>,\n        I: IntoIterator<Item = S>,\n        A: IntoIterator<Item = Aggregation>,\n    >(\n        self,\n        group_by: I,\n        aggregations: A,\n    ) -> Self {", "    /// **Optimization**: Accepts `IntoIterator` for both `group_by` and `aggregations`\n    /// to allow passing stack-allocated arrays instead of requiring heap-allocated vectors.\n    pub fn summarize<\n        S: Into<String>,\n        I: IntoIterator<Item = S>,\n        A: IntoIterator<Item = Aggregation>,\n    >(\n        self,\n        group_by: I,\n        aggregations: A,\n    ) -> Self {")

with open("relvar-core/src/query/mod.rs", "w") as f:
    f.write(content)
