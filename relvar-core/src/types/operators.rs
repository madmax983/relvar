//! User-defined scalar operators (TTM RM Prescription 3).
//!
//! This module implements the Third Manifesto's requirement that D provide
//! facilities for users to define their own scalar operators:
//!
//! > "D shall provide facilities for users to define and destroy their own
//! > scalar operators (user defined scalar operators)." — TTM RM Prescription 3
//!
//! A scalar operator is an operator that, when invoked, returns a scalar
//! value. The prescription imposes three requirements, each enforced here:
//!
//! - **3(a) — read-only (pure):** invoking an operator updates no variables
//!   other than ones local to its implementation. Registered implementations
//!   are therefore plain Rust closures over [`ScalarValue`]s with no access
//!   to the database; purity is a documented contract the author of the
//!   closure must honor (the type system cannot prove the absence of side
//!   effects, exactly as TTM notes for "determinate" operators).
//! - **3(b) — declared result type:** every invocation denotes a value of one
//!   declared type. The [`OperatorSignature`] records the return type and
//!   [`OperatorRegistry::invoke`] verifies the implementation's result
//!   against it.
//! - **3(c) — declared parameter types:** the definition specifies the type
//!   of each parameter, and each argument in an invocation must be of that
//!   type. [`OperatorRegistry::invoke`] resolves overloads by exact
//!   `(name, parameter types)` match and rejects mismatches.
//!
//! There are no NULLs anywhere in this design (TTM Proscription 1):
//! operators are total functions over [`ScalarValue`]; a domain violation is
//! a failed invocation ([`OperatorError`]), never a null.
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{OperatorRegistry, OperatorSignature, ScalarType};
//! use relvar_core::values::ScalarValue;
//!
//! let mut registry = OperatorRegistry::new();
//! registry.register(
//!     OperatorSignature {
//!         name: "double".to_string(),
//!         param_types: vec![ScalarType::Int],
//!         return_type: ScalarType::Int,
//!     },
//!     |args| match args[0] {
//!         ScalarValue::Int(n) => Ok(ScalarValue::Int(n * 2)),
//!         _ => unreachable!("signature guarantees an Int argument"),
//!     },
//! ).unwrap();
//!
//! assert_eq!(
//!     registry.invoke("double", &[ScalarValue::Int(21)]).unwrap(),
//!     ScalarValue::Int(42)
//! );
//! ```

use super::ScalarType;
use crate::values::ScalarValue;
use serde::{Deserialize, Serialize};
use crate::collections::HashMap;
use core::fmt;
use alloc::sync::Arc;
use thiserror::Error;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// Errors from operator registration, resolution, and invocation.
#[derive(Debug, Error)]
pub enum OperatorError {
    /// An operator with this name is already registered for these parameter types.
    #[error("operator '{name}' is already registered for parameter types ({params})")]
    AlreadyRegistered {
        /// Operator name.
        name: String,
        /// Comma-separated parameter type names.
        params: String,
    },

    /// No operator with this name is registered at all.
    #[error("unknown operator '{name}' for argument types ({args})")]
    UnknownOperator {
        /// Operator name.
        name: String,
        /// Comma-separated argument type names.
        args: String,
    },

    /// An operator with this name exists, but no overload takes this many arguments.
    #[error("operator '{name}' expects {expected} argument(s), got {got}")]
    ArityMismatch {
        /// Operator name.
        name: String,
        /// Expected arity (of the first same-named overload).
        expected: usize,
        /// Actual argument count.
        got: usize,
    },

    /// An overload with this name and arity exists, but an argument has the wrong type.
    ///
    /// TTM RM Prescription 3(c): the argument denoting the value for a
    /// parameter must be of that parameter's declared type.
    #[error("operator '{name}': argument {index} has type {actual}, expected {expected}")]
    ArgumentTypeMismatch {
        /// Operator name.
        name: String,
        /// Zero-based argument position.
        index: usize,
        /// Declared parameter type name.
        expected: String,
        /// Actual argument type name.
        actual: String,
    },

    /// The implementation returned a value of a different type than declared.
    ///
    /// TTM RM Prescription 3(b): every invocation must denote a value of the
    /// operator's declared result type.
    #[error("operator '{name}' returned {actual}, but its declared return type is {expected}")]
    ResultTypeMismatch {
        /// Operator name.
        name: String,
        /// Declared return type name.
        expected: String,
        /// Actual result type name.
        actual: String,
    },

    /// The implementation itself reported a domain or evaluation failure.
    ///
    /// Note this is not a NULL (TTM Proscription 1): the invocation simply
    /// fails instead of producing a value.
    #[error("operator '{name}' failed: {reason}")]
    EvaluationFailed {
        /// Operator name.
        name: String,
        /// Human-readable reason.
        reason: String,
    },

    /// The operator name is empty.
    #[error("operator name must not be empty")]
    InvalidName,
}

/// The declared signature of a user-defined scalar operator.
///
/// TTM RM Prescription 3(b)(c): the definition of an operator includes the
/// declared type of each parameter and the declared result type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperatorSignature {
    /// Operator name, e.g. `"age"`.
    pub name: String,
    /// Declared type of each parameter, in order.
    pub param_types: Vec<ScalarType>,
    /// Declared result type of every invocation.
    pub return_type: ScalarType,
}

/// The implementation of a user-defined operator: a pure function over scalar values.
///
/// # Purity contract (TTM RM Prescription 3(a))
///
/// The closure MUST be a pure (determinate) function of its arguments:
/// no I/O, no mutation of captured state, no dependence on hidden inputs
/// such as the system clock or random number generators. A closure that
/// violates this contract (e.g. via interior mutability) is a TTM violation
/// the registry cannot detect statically; keep operator bodies honest.
pub type OperatorFn =
    Arc<dyn Fn(&[ScalarValue]) -> Result<ScalarValue, OperatorError> + Send + Sync>;

/// A registered operator: its declared signature plus its implementation.
#[derive(Clone)]
pub struct RegisteredOperator {
    signature: OperatorSignature,
    implementation: OperatorFn,
}

impl fmt::Debug for RegisteredOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredOperator")
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}

impl RegisteredOperator {
    /// The operator's declared signature.
    pub fn signature(&self) -> &OperatorSignature {
        &self.signature
    }
}

/// Registry key: operator name plus exact parameter types (overloading).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OperatorKey {
    name: String,
    param_types: Vec<ScalarType>,
}

impl From<&OperatorSignature> for OperatorKey {
    fn from(signature: &OperatorSignature) -> Self {
        Self {
            name: signature.name.clone(),
            param_types: signature.param_types.clone(),
        }
    }
}

/// Renders a type list as `"Int, String"` for error messages.
fn format_types(types: &[ScalarType]) -> String {
    types
        .iter()
        .map(ScalarType::name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Registry of user-defined scalar operators, keyed by `(name, parameter types)`.
///
/// This is the v1 realization of TTM RM Prescription 3's "facilities for
/// users to define and destroy their own scalar operators": [`register`](Self::register)
/// defines, [`unregister`](Self::unregister) destroys, and
/// [`invoke`](Self::invoke) applies the prescription's type discipline.
#[derive(Debug, Clone, Default)]
pub struct OperatorRegistry {
    operators: HashMap<OperatorKey, RegisteredOperator>,
}

impl OperatorRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            operators: HashMap::new(),
        }
    }

    /// Defines a new user-defined scalar operator (TTM RM Prescription 3).
    ///
    /// The same name may be registered multiple times with different
    /// parameter types (overloading); registering the exact same
    /// `(name, parameter types)` twice is an error.
    ///
    /// # Errors
    ///
    /// - [`OperatorError::InvalidName`] if the signature's name is empty.
    /// - [`OperatorError::AlreadyRegistered`] if this exact signature exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{OperatorRegistry, OperatorSignature, ScalarType};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let mut registry = OperatorRegistry::new();
    /// registry.register(
    ///     OperatorSignature {
    ///         name: "is_adult".to_string(),
    ///         param_types: vec![ScalarType::Int],
    ///         return_type: ScalarType::Bool,
    ///     },
    ///     |args| match args[0] {
    ///         ScalarValue::Int(age) => Ok(ScalarValue::Bool(age >= 18)),
    ///         _ => unreachable!("signature guarantees an Int argument"),
    ///     },
    /// ).unwrap();
    /// ```
    pub fn register(
        &mut self,
        signature: OperatorSignature,
        implementation: impl Fn(&[ScalarValue]) -> Result<ScalarValue, OperatorError>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), OperatorError> {
        if signature.name.is_empty() {
            return Err(OperatorError::InvalidName);
        }
        let key = OperatorKey::from(&signature);
        if self.operators.contains_key(&key) {
            return Err(OperatorError::AlreadyRegistered {
                name: signature.name.clone(),
                params: format_types(&signature.param_types),
            });
        }
        self.operators.insert(
            key,
            RegisteredOperator {
                signature,
                implementation: Arc::new(implementation),
            },
        );
        Ok(())
    }

    /// Destroys a user-defined scalar operator (TTM RM Prescription 3:
    /// "facilities for users to define and destroy").
    ///
    /// # Errors
    ///
    /// [`OperatorError::UnknownOperator`] if no such `(name, parameter types)`
    /// registration exists.
    pub fn unregister(
        &mut self,
        name: &str,
        param_types: &[ScalarType],
    ) -> Result<(), OperatorError> {
        let key = OperatorKey {
            name: name.to_string(),
            param_types: param_types.to_vec(),
        };
        self.operators
            .remove(&key)
            .map(|_| ())
            .ok_or_else(|| OperatorError::UnknownOperator {
                name: name.to_string(),
                args: format_types(param_types),
            })
    }

    /// Resolves an operator by name and exact argument types (overload resolution).
    ///
    /// Returns `None` when no registered operator matches.
    pub fn resolve(&self, name: &str, arg_types: &[ScalarType]) -> Option<&RegisteredOperator> {
        let key = OperatorKey {
            name: name.to_string(),
            param_types: arg_types.to_vec(),
        };
        self.operators.get(&key)
    }

    /// Returns `true` if an operator is registered for this name and parameter types.
    pub fn contains(&self, name: &str, param_types: &[ScalarType]) -> bool {
        self.resolve(name, param_types).is_some()
    }

    /// Invokes a registered operator on argument values.
    ///
    /// Overload resolution is by exact `(name, argument types)` match, per
    /// TTM RM Prescription 3(c). The implementation's result is checked
    /// against the declared return type, per 3(b).
    ///
    /// # Errors
    ///
    /// - [`OperatorError::UnknownOperator`] if no operator with this name exists.
    /// - [`OperatorError::ArityMismatch`] if the name exists but no overload
    ///   takes this many arguments.
    /// - [`OperatorError::ArgumentTypeMismatch`] if an overload with this
    ///   name and arity exists but an argument has the wrong type.
    /// - [`OperatorError::ResultTypeMismatch`] if the implementation
    ///   returned a value of the wrong type.
    /// - [`OperatorError::EvaluationFailed`] if the implementation failed.
    pub fn invoke(&self, name: &str, args: &[ScalarValue]) -> Result<ScalarValue, OperatorError> {
        let arg_types: Vec<ScalarType> = args.iter().map(ScalarValue::scalar_type).collect();

        if let Some(operator) = self.resolve(name, &arg_types) {
            let result = (operator.implementation)(args)?;
            let result_type = result.scalar_type();
            if result_type != operator.signature.return_type {
                return Err(OperatorError::ResultTypeMismatch {
                    name: name.to_string(),
                    expected: operator.signature.return_type.name().to_string(),
                    actual: result_type.name().to_string(),
                });
            }
            return Ok(result);
        }

        // No exact match: produce the most helpful diagnostic.
        let mut same_name: Vec<&RegisteredOperator> = self
            .operators
            .values()
            .filter(|op| op.signature.name == name)
            .collect();
        if same_name.is_empty() {
            return Err(OperatorError::UnknownOperator {
                name: name.to_string(),
                args: format_types(&arg_types),
            });
        }
        same_name.sort_by(|a, b| {
            a.signature
                .param_types
                .len()
                .cmp(&b.signature.param_types.len())
        });
        if let Some(candidate) = same_name
            .iter()
            .find(|op| op.signature.param_types.len() == args.len())
        {
            for (index, (expected, actual)) in candidate
                .signature
                .param_types
                .iter()
                .zip(arg_types.iter())
                .enumerate()
            {
                if expected != actual {
                    return Err(OperatorError::ArgumentTypeMismatch {
                        name: name.to_string(),
                        index,
                        expected: expected.name().to_string(),
                        actual: actual.name().to_string(),
                    });
                }
            }
        }
        Err(OperatorError::ArityMismatch {
            name: name.to_string(),
            expected: same_name[0].signature.param_types.len(),
            got: args.len(),
        })
    }

    /// Number of registered operators (counting each overload separately).
    pub fn len(&self) -> usize {
        self.operators.len()
    }

    /// Returns `true` if no operators are registered.
    pub fn is_empty(&self) -> bool {
        self.operators.is_empty()
    }

    /// All registered signatures.
    pub fn signatures(&self) -> Vec<&OperatorSignature> {
        self.operators.values().map(|op| &op.signature).collect()
    }
}

#[cfg(test)]
#[path = "operators_tests.rs"]
mod operators_tests;
