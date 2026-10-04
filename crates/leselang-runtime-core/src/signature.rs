use std::{fmt, iter::FusedIterator};

/// Native host parameter metadata with a developer-owned key and opaque domain.
///
/// The domain may describe scalar types or host constraints; this module never
/// interprets or executes it. `required` means the key must be supplied, not that
/// its eventual value must be non-null. This is mutable metadata, not a registered
/// operation, capability grant, versioned wire schema or validation certificate.
/// Constructors do not check key grammar, uniqueness, domains or resource limits.
/// `Debug` may expose private metadata; only the error type below is payload-free.
///
/// ```compile_fail
/// use leselang_runtime_core::NamedParameter;
/// let wire = serde_json::to_string(&NamedParameter::required("target", ())).unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::NamedParameter;
/// let parameter: NamedParameter<String, ()> = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedParameter<Key, Domain> {
    pub name: Key,
    pub domain: Domain,
    pub required: bool,
}

impl<Key, Domain> NamedParameter<Key, Domain> {
    pub const fn required(name: Key, domain: Domain) -> Self {
        Self {
            name,
            domain,
            required: true,
        }
    }

    pub const fn optional(name: Key, domain: Domain) -> Self {
        Self {
            name,
            domain,
            required: false,
        }
    }
}

/// Fixed error tags and positions, never submitted keys or host-domain payloads.
/// Indices refer to the supplied slices, not executable or restorable handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedArgumentError {
    TooManyArguments,
    DuplicateParameter { index: usize },
    DuplicateArgument { index: usize },
    UnknownArgument { index: usize },
    MissingArgument { parameter_index: usize },
}

impl fmt::Display for NamedArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooManyArguments => "named argument count exceeds parameter count",
            Self::DuplicateParameter { .. } => "duplicate parameter key in host signature",
            Self::DuplicateArgument { .. } => "duplicate named argument key",
            Self::UnknownArgument { .. } => "unknown named argument key",
            Self::MissingArgument { .. } => "missing required named argument",
        })
    }
}

impl std::error::Error for NamedArgumentError {}

/// Borrowed named-argument shape preflight, independent of host operations and values.
///
/// Order is count, duplicate schema keys, left-to-right submitted keys (duplicate
/// before unknown at each position), then required parameters in declaration order.
/// Supplied keys may be reordered; optional keys may be absent. Key comparison uses
/// `PartialEq`, with no Clone, Hash, Debug, serde or Send bound on keys or domains.
/// Domains are not read, values are not evaluated, and nothing is sorted or filled in.
///
/// There is no allocation, collection, iterator size hint, callback to a value
/// evaluator or global lock. Work is quadratic in the caller-bounded slice lengths;
/// hosts must bound ingress bytes and schema/argument counts before calling. Native
/// key comparisons can be expensive, mutate interior state or unwind; their effects
/// are not preempted or rolled back. They must obey a stable equivalence relation.
///
/// Success observes these keys/required flags at this instant, not later mutations,
/// value types/bounds/domains, schema version compatibility, authority or execution.
/// The host still selects an operation and performs those checks before dispatch.
///
/// ```
/// use leselang_runtime_core::{NamedArgumentError, NamedParameter, validate_named_arguments};
/// #[derive(PartialEq)]
/// enum Key { Target, Caption }
/// enum Domain { Node, Text }
/// let schema = [
///     NamedParameter::required(Key::Target, Domain::Node),
///     NamedParameter::optional(Key::Caption, Domain::Text),
/// ];
/// validate_named_arguments(&[Key::Caption, Key::Target], &schema).unwrap();
/// validate_named_arguments(&[Key::Target], &schema).unwrap();
/// assert_eq!(validate_named_arguments(&[], &schema),
///     Err(NamedArgumentError::MissingArgument { parameter_index: 0 }));
/// ```
pub fn validate_named_arguments<Key: PartialEq, Domain>(
    names: &[Key],
    parameters: &[NamedParameter<Key, Domain>],
) -> Result<(), NamedArgumentError> {
    if names.len() > parameters.len() {
        return Err(NamedArgumentError::TooManyArguments);
    }
    if let Some(index) = duplicate_named_parameter_index(parameters) {
        return Err(NamedArgumentError::DuplicateParameter { index });
    }
    for (index, name) in names.iter().enumerate() {
        if names[..index].contains(name) {
            return Err(NamedArgumentError::DuplicateArgument { index });
        }
        if !parameters.iter().any(|parameter| parameter.name == *name) {
            return Err(NamedArgumentError::UnknownArgument { index });
        }
    }
    for (parameter_index, parameter) in parameters.iter().enumerate() {
        if parameter.required && !names.contains(&parameter.name) {
            return Err(NamedArgumentError::MissingArgument { parameter_index });
        }
    }
    Ok(())
}

pub(crate) fn duplicate_named_parameter_index<Key: PartialEq, Domain>(
    parameters: &[NamedParameter<Key, Domain>],
) -> Option<usize> {
    parameters
        .iter()
        .enumerate()
        .find_map(|(index, parameter)| {
            parameters[..index]
                .iter()
                .any(|previous| previous.name == parameter.name)
                .then_some(index)
        })
}

/// Borrowed, preflighted name-to-input alignment, never evaluated argument values.
///
/// Forward iteration follows parameter declaration order and omits absent optional
/// parameters. Each item borrows its original parameter and reports the matching
/// index in the original names slice. No schema, key or value is cloned, reordered
/// or filled in. The host must pair indices with that same submission's values.
/// This is not a type/domain certificate, operation registration, dispatch grant,
/// saved frame or one-shot execution handle; iteration may be repeated.
///
/// Direct mutation of the borrowed names/schema is fenced by Rust, not interior
/// mutation or host version/authority policy. Native key comparison must remain a
/// stable equivalence relation throughout use. Iteration compares keys again; its
/// callbacks can be expensive, mutate interior state or unwind. They are not
/// preempted or rolled back. Hosts bound counts/bytes and perform full value,
/// domain and authority preflight before publishing effects.
///
/// Debug reports only slice counts, without key/domain formatters. The view has
/// no serde codec, owned snapshot or Clone/Send bound on host metadata. Thread
/// transfer is conditional on borrowed metadata being Sync, not a global lock.
///
/// ```compile_fail
/// use leselang_runtime_core::{NamedParameter, bind_named_arguments};
/// let mut names = ["target"];
/// let parameters = [NamedParameter::required("target", ())];
/// let bindings = bind_named_arguments(&names, &parameters).unwrap();
/// names[0] = "other";
/// let _ = bindings.iter().next();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{NamedParameter, bind_named_arguments};
/// let names = ["target"];
/// let mut parameters = [NamedParameter::required("target", ())];
/// let bindings = bind_named_arguments(&names, &parameters).unwrap();
/// parameters[0].required = false;
/// let _ = bindings.iter().next();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{NamedParameter, bind_named_arguments};
/// let names = ["target"];
/// let parameters = [NamedParameter::required("target", ())];
/// let bindings = bind_named_arguments(&names, &parameters).unwrap();
/// let wire = serde_json::to_string(&bindings).unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::NamedArgumentBindings;
/// let bindings: NamedArgumentBindings<'_, String, ()> = serde_json::from_str("{}").unwrap();
/// ```
pub struct NamedArgumentBindings<'a, Key, Domain> {
    names: &'a [Key],
    parameters: &'a [NamedParameter<Key, Domain>],
}

impl<'a, Key: PartialEq, Domain> NamedArgumentBindings<'a, Key, Domain> {
    /// Allocation-free forward declaration order; reverse traversal is metadata
    /// inspection only, not permission to change an evaluator's execution order.
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&'a NamedParameter<Key, Domain>, usize)> + FusedIterator + '_
    {
        self.parameters.iter().filter_map(|parameter| {
            self.names
                .iter()
                .position(|name| name == &parameter.name)
                .map(|index| (parameter, index))
        })
    }
}

impl<Key, Domain> fmt::Debug for NamedArgumentBindings<'_, Key, Domain> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NamedArgumentBindings")
            .field("arguments", &self.names.len())
            .field("parameters", &self.parameters.len())
            .finish()
    }
}

/// Apply the existing named-shape preflight before constructing a borrowed view.
///
/// Error priority/positions are exactly `validate_named_arguments`. Construction
/// and traversal involve no core allocation, domain reads or value evaluation.
/// Native key comparisons may themselves allocate or execute arbitrary code.
/// Preflight is quadratic; each traversal is at most names.len() * parameters.len()
/// native comparisons. No caller-provided iterator or size hint is consumed.
/// Optional omission creates no value/default; required presence is not non-nullness.
///
/// ```
/// use leselang_runtime_core::{NamedParameter, bind_named_arguments};
/// let names = ["caption", "target"];
/// let parameters = [
///     NamedParameter::required("target", ()),
///     NamedParameter::optional("enabled", ()),
///     NamedParameter::optional("caption", ()),
/// ];
/// let bindings = bind_named_arguments(&names, &parameters).unwrap();
/// let aligned: Vec<_> = bindings.iter().map(|(parameter, index)| (parameter.name, index)).collect();
/// assert_eq!(aligned, [("target", 1), ("caption", 0)]);
/// ```
pub fn bind_named_arguments<'a, Key: PartialEq, Domain>(
    names: &'a [Key],
    parameters: &'a [NamedParameter<Key, Domain>],
) -> Result<NamedArgumentBindings<'a, Key, Domain>, NamedArgumentError> {
    validate_named_arguments(names, parameters)?;
    Ok(NamedArgumentBindings { names, parameters })
}
