//! Owned program assembly with explicit native entry lowering and admission.

use std::fmt;

use leselang_syntax::{Function, Span};

use crate::helper_body::HelperBody;
use crate::helper_declarations::HelperDeclarations;
use crate::helper_registry::{HelperRegistryError, HelperRegistryLimits, assemble_helper_registry};
use crate::ir::Computation;
use crate::source_call::{
    SourceCallError, SourceCallLimits, SourceShapeError, preflight, valid_limits,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramSourceLimits {
    pub helpers: HelperRegistryLimits,
    pub max_entry_source_nodes: usize,
    pub max_entry_source_depth: usize,
    pub max_entry_arguments: usize,
}

pub enum ProgramSourceError<Lowering, Registration, Entry, Admission> {
    InvalidLimits,
    Source { span: Span, error: SourceShapeError },
    Helpers(HelperRegistryError<Lowering, Registration>),
    Entry { span: Span, error: Entry },
    Admission { span: Span, error: Admission },
}
impl<Lowering, Registration, Entry, Admission> fmt::Display
    for ProgramSourceError<Lowering, Registration, Entry, Admission>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "program entry source limits exceed safety ceilings",
            Self::Source { .. } => "program entry physical source is invalid",
            Self::Helpers(_) => "program helper registry assembly failed",
            Self::Entry { .. } => "native program entry lowering failed",
            Self::Admission { .. } => "native whole program admission failed",
        })
    }
}
impl<Lowering, Registration, Entry, Admission> fmt::Debug
    for ProgramSourceError<Lowering, Registration, Entry, Admission>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Lowering, Registration, Entry, Admission> std::error::Error
    for ProgramSourceError<Lowering, Registration, Entry, Admission>
{
}

/// Original borrowed entry, owned native output and its fully assembled state.
/// No executable/type/authority certificate is implied. Native interior state
/// can change after admission; fresh live policy is mandatory before use.
#[must_use = "retain or deliberately consume the admitted program assembly"]
pub struct ProgramSource<'source, Output, State> {
    entry: &'source Function,
    output: Output,
    state: State,
}
impl<Output, State> fmt::Debug for ProgramSource<'_, Output, State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProgramSource")
    }
}
impl<'source, Output, State> ProgramSource<'source, Output, State> {
    pub fn entry(&self) -> &'source Function {
        self.entry
    }
    pub fn output(&self) -> &Output {
        &self.output
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn into_parts(self) -> (&'source Function, Output, State) {
        (self.entry, self.output, self.state)
    }
}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type ProgramSourceResult<'source, Output, State, Lowering, Registration, Entry, Admission> =
    Result<
        ProgramSource<'source, Output, State>,
        ProgramSourceError<Lowering, Registration, Entry, Admission>,
    >;

fn entry_limits(limits: ProgramSourceLimits) -> SourceCallLimits {
    SourceCallLimits {
        max_source_nodes: limits.max_entry_source_nodes,
        max_source_depth: limits.max_entry_source_depth,
        max_lowered_nodes: 0,
        max_lowered_depth: 0,
        max_arguments: limits.max_entry_arguments,
    }
}

/// Assemble an admitted declaration forest, not parse source or implement native
/// recursive lowering. Entry selection/parameterless signatures come only from
/// the original HelperDeclarations. Complete cold entry physical names, text,
/// arity and bounds pass before callbacks; the shared helper registry then applies
/// its current ceilings, whole helper source checks and lazy prepare/register
/// gates. Unused helpers are not pruned. A helper error prevents entry lowering.
///
/// Lower the designated original entry exactly once against the completed owned
/// state, then admit the complete original output exactly once against that same
/// state. Native output is opaque: the host MUST bound generated output/native
/// graphs before allocation growth, corroborate cold lexical/types/result/cost,
/// faithful source accounting, canonical bytes, schemas/versions/grants and any
/// reported capability list in these hooks. Source observations and successful
/// admission do not authorize unknown calls, execution or reply acceptance.
///
/// Return entry/output/state together only after admission succeeds. Failures and
/// unwind drop core-owned output/state without partial handoff, retry, implicit
/// copies, native payload formatting/source chains or reservation refunds. This
/// is not transactional rollback of external storage or native side effects.
/// State may move between assembly stages; no stable-address or pinning promise
/// is made. Self-referential native state needs caller-owned pinned allocation.
/// Caller aliases/interior mutability, native ingress/work/allocation/Drop/unwind
/// stay trusted host responsibilities. Hooks must not dispatch or publish runtime
/// authority. No source counter, fuel, global registry, cache policy, lock, codec,
/// execution, journal or durable restart is installed. This is shared program
/// assembly, not a complete generic recursive compiler or independent VM.
pub fn assemble_program<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    State,
    Output,
    Lowering,
    Registration,
    Entry,
    Admission,
>(
    declarations: &HelperDeclarations<'source>,
    limits: ProgramSourceLimits,
    state: State,
    prepare: impl FnMut(
        &'source Function,
        &mut State,
    ) -> Result<
        HelperBody<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        Lowering,
    >,
    register: impl FnMut(
        &'source Function,
        HelperBody<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        &mut State,
    ) -> Result<(), Registration>,
    lower_entry: impl FnOnce(&'source Function, &mut State) -> Result<Output, Entry>,
    admit: impl FnOnce(&'source Function, &Output, &mut State) -> Result<(), Admission>,
) -> ProgramSourceResult<'source, Output, State, Lowering, Registration, Entry, Admission> {
    let source = entry_limits(limits);
    if !valid_limits(source) {
        return Err(ProgramSourceError::InvalidLimits);
    }
    preflight::<Entry>(&declarations.entry().body, source).map_err(|error| match error {
        SourceCallError::Shape { span, error } => ProgramSourceError::Source { span, error },
        _ => ProgramSourceError::InvalidLimits,
    })?;
    assemble_preflighted_program(
        declarations,
        limits,
        state,
        prepare,
        register,
        lower_entry,
        admit,
    )
}

// The product parser/declaration boundary already bounds entry source. Keep its
// ready-helper diagnostic priority while sharing complete-state entry admission.
pub(crate) fn assemble_preflighted_program<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    State,
    Output,
    Lowering,
    Registration,
    Entry,
    Admission,
>(
    declarations: &HelperDeclarations<'source>,
    limits: ProgramSourceLimits,
    state: State,
    prepare: impl FnMut(
        &'source Function,
        &mut State,
    ) -> Result<
        HelperBody<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        Lowering,
    >,
    register: impl FnMut(
        &'source Function,
        HelperBody<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        &mut State,
    ) -> Result<(), Registration>,
    lower_entry: impl FnOnce(&'source Function, &mut State) -> Result<Output, Entry>,
    admit: impl FnOnce(&'source Function, &Output, &mut State) -> Result<(), Admission>,
) -> ProgramSourceResult<'source, Output, State, Lowering, Registration, Entry, Admission> {
    if !valid_limits(entry_limits(limits)) {
        return Err(ProgramSourceError::InvalidLimits);
    }
    let mut state =
        assemble_helper_registry(declarations, limits.helpers, state, prepare, register)
            .map_err(ProgramSourceError::Helpers)?;
    let entry = declarations.entry();
    let span = entry.span;
    let output = lower_entry(entry, &mut state)
        .map_err(|error| ProgramSourceError::Entry { span, error })?;
    admit(entry, &output, &mut state)
        .map_err(|error| ProgramSourceError::Admission { span, error })?;
    Ok(ProgramSource {
        entry,
        output,
        state,
    })
}
