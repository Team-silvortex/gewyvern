use std::{borrow::Borrow, fmt};

use crate::signature::duplicate_named_parameter_index;
use crate::{NamedArgumentBindings, NamedArgumentError, NamedParameter, bind_named_arguments};

/// Native operation declaration with host-owned keys, domains, result and capability tags.
///
/// Public fields are unchecked metadata, not executable callbacks, authority,
/// schema compatibility proof or a wire codec. Result/domain descriptors are
/// opaque to registration/lookup/binding. Scalar argument preflight separately
/// opts domains into `ScalarArgumentDomain`. Debug reports only parameter count.
/// Hosts bound key bytes/native work and supply versioned protocol adapters.
///
/// ```compile_fail
/// use leselang_runtime_core::OperationSchema;
/// let schema: OperationSchema<'_, String, String, (), (), ()> = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::OperationSchema;
/// let schema: OperationSchema<'_, &str, &str, (), (), ()> = OperationSchema {
///     key: "observe", parameters: &[], result: (), required_capability: () };
/// let wire = serde_json::to_string(&schema).unwrap();
/// ```
pub struct OperationSchema<'a, Key, Parameter, Domain, Result, Capability> {
    pub key: Key,
    pub parameters: &'a [NamedParameter<Parameter, Domain>],
    pub result: Result,
    pub required_capability: Capability,
}

impl<Key, Parameter: PartialEq, Domain, Result, Capability>
    OperationSchema<'_, Key, Parameter, Domain, Result, Capability>
{
    pub fn bind_arguments<'a>(
        &'a self,
        names: &'a [Parameter],
    ) -> std::result::Result<NamedArgumentBindings<'a, Parameter, Domain>, NamedArgumentError> {
        bind_named_arguments(names, self.parameters)
    }
}

impl<Key, Parameter, Domain, Result, Capability> fmt::Debug
    for OperationSchema<'_, Key, Parameter, Domain, Result, Capability>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationSchema")
            .field("parameters", &self.parameters.len())
            .finish()
    }
}

/// Explicit inclusive metadata count limits, not evaluation fuel or a memory quota.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationCatalogLimits {
    pub max_operations: usize,
    pub max_parameters_per_operation: usize,
}

/// Payload-free metadata/lookup failures. Indices are positions, not operation IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationCatalogError {
    InvalidVersion,
    OperationLimit,
    ParameterLimit {
        operation_index: usize,
    },
    DuplicateOperation {
        operation_index: usize,
    },
    DuplicateParameter {
        operation_index: usize,
        parameter_index: usize,
    },
    UnsupportedVersion,
    UnknownOperation,
    CapabilityDenied,
}

impl fmt::Display for OperationCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidVersion => "operation catalog version must be nonzero",
            Self::OperationLimit => "operation catalog exceeds its operation limit",
            Self::ParameterLimit { .. } => "operation schema exceeds its parameter limit",
            Self::DuplicateOperation { .. } => "duplicate operation key in catalog",
            Self::DuplicateParameter { .. } => "duplicate parameter key in operation schema",
            Self::UnsupportedVersion => "operation catalog version does not match",
            Self::UnknownOperation => "unknown operation key",
            Self::CapabilityDenied => "operation capability is not granted",
        })
    }
}

impl std::error::Error for OperationCatalogError {}

/// Version-pinned borrowed declarations, independent of product opcodes and storage.
///
/// Construction checks nonzero version, operation count, every parameter count,
/// duplicate operation keys, then duplicate parameter keys. No domain, result or
/// capability tag is validated/read; optional and required parameters are both
/// declarations. Limits are host policy and can be zero (an explicit deny-all).
///
/// This validates native metadata, not language evaluation, result authenticity,
/// stable wire compatibility or permission to execute. The borrow fences direct
/// schema mutation, not interior state. Native equality/Borrow must retain stable
/// equivalent keys and may allocate, run code or unwind without rollback or
/// preemption. Core-owned work allocates nothing; registration is quadratic in
/// caller-bounded counts, lookup is linear. Hosts bound key bytes and native work.
///
/// No schema/keys/domains/results/capabilities are cloned or formatted. Debug
/// reports version and operation count only. There is no implicit default, serde
/// handle, global registry, dynamic library loading or persistence backend.
///
/// ```compile_fail
/// use leselang_runtime_core::OperationCatalog;
/// let catalog: OperationCatalog<'_, String, String, (), (), ()> = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{OperationCatalog, OperationCatalogLimits, OperationSchema};
/// let schemas = [OperationSchema { key: "observe", parameters: &[], result: (), required_capability: () }];
/// let catalog = OperationCatalog::<_, &str, (), (), ()>::new(1, &schemas,
///     OperationCatalogLimits { max_operations: 1, max_parameters_per_operation: 0 }).unwrap();
/// let wire = serde_json::to_string(&catalog).unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{OperationCatalog, OperationCatalogLimits, OperationSchema};
/// let mut schemas: [OperationSchema<'_, &str, &str, (), (), ()>; 1] = [OperationSchema {
///     key: "observe", parameters: &[], result: (), required_capability: () }];
/// let catalog = OperationCatalog::new(1, &schemas,
///     OperationCatalogLimits { max_operations: 1, max_parameters_per_operation: 0 }).unwrap();
/// schemas[0].key = "changed";
/// catalog.lookup("observe", 1).unwrap();
/// ```
///
/// ```compile_fail
/// use std::rc::Rc;
/// use leselang_runtime_core::{OperationCatalog, OperationCatalogLimits, OperationSchema};
/// fn requires_send<T: Send>(_: T) {}
/// let schemas: [OperationSchema<'_, &str, &str, (), Rc<()>, ()>; 1] = [OperationSchema {
///     key: "observe", parameters: &[], result: Rc::new(()), required_capability: () }];
/// let catalog = OperationCatalog::new(1, &schemas,
///     OperationCatalogLimits { max_operations: 1, max_parameters_per_operation: 0 }).unwrap();
/// requires_send(catalog);
/// ```
pub struct OperationCatalog<'a, Key, Parameter, Domain, Result, Capability> {
    version: u32,
    schemas: &'a [OperationSchema<'a, Key, Parameter, Domain, Result, Capability>],
}

impl<'a, Key: PartialEq, Parameter: PartialEq, Domain, Result, Capability>
    OperationCatalog<'a, Key, Parameter, Domain, Result, Capability>
{
    pub fn new(
        version: u32,
        schemas: &'a [OperationSchema<'a, Key, Parameter, Domain, Result, Capability>],
        limits: OperationCatalogLimits,
    ) -> std::result::Result<Self, OperationCatalogError> {
        if version == 0 {
            return Err(OperationCatalogError::InvalidVersion);
        }
        if schemas.len() > limits.max_operations {
            return Err(OperationCatalogError::OperationLimit);
        }
        for (operation_index, schema) in schemas.iter().enumerate() {
            if schema.parameters.len() > limits.max_parameters_per_operation {
                return Err(OperationCatalogError::ParameterLimit { operation_index });
            }
        }
        for (operation_index, schema) in schemas.iter().enumerate() {
            if schemas[..operation_index]
                .iter()
                .any(|previous| previous.key == schema.key)
            {
                return Err(OperationCatalogError::DuplicateOperation { operation_index });
            }
        }
        for (operation_index, schema) in schemas.iter().enumerate() {
            if let Some(parameter_index) = duplicate_named_parameter_index(schema.parameters) {
                return Err(OperationCatalogError::DuplicateParameter {
                    operation_index,
                    parameter_index,
                });
            }
        }
        Ok(Self { version, schemas })
    }
}

impl<'a, Key, Parameter, Domain, Result, Capability>
    OperationCatalog<'a, Key, Parameter, Domain, Result, Capability>
{
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Exact version before native lookup. Borrow permits an owned string key to
    /// be queried by str, without constructing/cloning a key or normalizing it.
    /// Success returns the original declaration, not approval of argument values.
    ///
    /// ```
    /// use leselang_runtime_core::{NamedParameter, OperationCatalog, OperationCatalogLimits, OperationSchema, ScalarType};
    /// let parameters = [NamedParameter::required("position", ScalarType::Integer)];
    /// let schemas = [OperationSchema { key: String::from("device.move"), parameters: &parameters,
    ///     result: ScalarType::Boolean, required_capability: "motion" }];
    /// let catalog = OperationCatalog::new(7, &schemas, OperationCatalogLimits {
    ///     max_operations: 1, max_parameters_per_operation: 1 }).unwrap();
    /// let schema = catalog.authorize("device.move", 7, &["motion"]).unwrap();
    /// assert_eq!(schema.result, ScalarType::Boolean);
    /// schema.bind_arguments(&["position"]).unwrap();
    /// ```
    pub fn lookup<Query: PartialEq + ?Sized>(
        &self,
        key: &Query,
        version: u32,
    ) -> std::result::Result<
        &'a OperationSchema<'a, Key, Parameter, Domain, Result, Capability>,
        OperationCatalogError,
    >
    where
        Key: Borrow<Query>,
    {
        if version != self.version {
            return Err(OperationCatalogError::UnsupportedVersion);
        }
        self.schemas
            .iter()
            .find(|schema| schema.key.borrow() == key)
            .ok_or(OperationCatalogError::UnknownOperation)
    }

    /// Exact version, known operation, then required-label membership, before
    /// any value binding. Granted labels are trusted host data, not script grants.
    /// This is metadata preflight only: no resource/revision/deadline/receipt
    /// check, host operation invocation, dispatch or durable replay is performed.
    /// Native capability equality has the same bounded-work/unwind obligations.
    pub fn authorize<Query: PartialEq + ?Sized>(
        &self,
        key: &Query,
        version: u32,
        granted: &[Capability],
    ) -> std::result::Result<
        &'a OperationSchema<'a, Key, Parameter, Domain, Result, Capability>,
        OperationCatalogError,
    >
    where
        Key: Borrow<Query>,
        Capability: PartialEq,
    {
        let schema = self.lookup(key, version)?;
        if !granted.contains(&schema.required_capability) {
            return Err(OperationCatalogError::CapabilityDenied);
        }
        Ok(schema)
    }
}

impl<Key, Parameter, Domain, Result, Capability> fmt::Debug
    for OperationCatalog<'_, Key, Parameter, Domain, Result, Capability>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationCatalog")
            .field("version", &self.version)
            .field("operations", &self.schemas.len())
            .finish()
    }
}
