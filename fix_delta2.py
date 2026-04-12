import re

filepath = 'relvar-core/src/algebra/delta.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace(".map_err(|e: crate::error::AlgebraError| DatabaseError::AlgebraError(e.to_string()))", ".map_err(|e| DatabaseError::AlgebraError(e.to_string()))")
content = content.replace("use crate::error::AlgebraError;\n", "")
content = content.replace("use crate::error::DatabaseError;", "use crate::DatabaseError;")
content = content.replace(".map_err(|e| DatabaseError::AlgebraError(e.to_string()))", ".map_err(|e: crate::error::RelationError| DatabaseError::AlgebraError(e.to_string()))")
content = content.replace(".map_err(|e: crate::error::RelationError| DatabaseError::AlgebraError(e.to_string()))", ".map_err(|e: crate::error::TupleError| DatabaseError::AlgebraError(e.to_string()))", 2)

with open(filepath, 'w') as f:
    f.write(content)
