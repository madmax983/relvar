import sys

with open("relvar-core/src/algebra/mod.rs", 'r') as f:
    content = f.read()

import re
content = re.sub(r'//! ```rust\n//! use relvar_core::types::{RelationType, ScalarType, TupleType};\n//! use relvar_core::values::{Relation, Tuple};\n//! use relvar_core::tuple;\n//!\n//! let r1 = Relation::new\(RelationType::new\(\n//!     TupleType::new\(\)\.with_attribute\("id", ScalarType::Int\)\n//! \)\);\n//!\n//! let r2 = Relation::new\(RelationType::new\(\n//!     TupleType::new\(\)\.with_attribute\("id", ScalarType::Int\)\n//! \)\);\n//!\n//! let delta = Delta::between\(&r1, &r2\)\.unwrap\(\);\n//! ```', '', content)

with open("relvar-core/src/algebra/mod.rs", 'w') as f:
    f.write(content)
