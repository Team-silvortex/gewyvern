//! Frozen pre-stage argument loops; shared shape/type kernels are intentionally
//! unchanged. This compares binding/lowering order, faults and output, not a
//! second compiler or independent type interpreter.
use crate::helper_source::*;
use crate::ir::Computation;
use crate::source_call::*;
use leselang_runtime_core::*;
use leselang_syntax::{Expression, NamedArgument, Span};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod frozen_helper {
    use super::*;
    use crate::pure_typing::{PureTypeError, preflight_with_budget, valid_local_name};
    use crate::source_call::valid_limits;
    use leselang_syntax::MAX_FUNCTION_PARAMETERS;
    type Node<F, O, H, R> = Computation<F, O, H, R>;
    pub(super) fn lower<'source, 'signature, Field, Operation, HostEffect, IrResult, Error>(
        arguments: &'source [NamedArgument],
        parameters: &'signature [HelperParameter<'signature>],
        limits: HelperSourceLimits,
        mut lower: impl FnMut(
            &'source NamedArgument,
        ) -> Result<
            (
                Node<Field, Operation, HostEffect, IrResult>,
                Option<ScalarType>,
            ),
            Error,
        >,
    ) -> HelperSourceResult<'source, 'signature, Node<Field, Operation, HostEffect, IrResult>, Error>
    {
        if !valid_limits(limits.source) || limits.max_parameters > MAX_FUNCTION_PARAMETERS {
            return Err(HelperSourceError::InvalidLimits);
        }
        if parameters.len() > limits.max_parameters {
            return Err(HelperSourceError::ParameterLimit);
        }
        if arguments.len() > limits.max_parameters || arguments.len() > limits.source.max_arguments
        {
            return Err(HelperSourceError::ArgumentLimit);
        }
        for (index, parameter) in parameters.iter().enumerate() {
            if !parameter.required || !valid_local_name(parameter.name) {
                return Err(HelperSourceError::InvalidParameter { index });
            }
        }
        let names = arguments
            .iter()
            .map(|argument| argument.name.as_str())
            .collect::<Vec<_>>();
        let bound = bind_named_arguments(&names, parameters).map_err(HelperSourceError::Names)?;
        let mut budget = StructureBudget::new(
            limits.source.max_lowered_nodes,
            limits.source.max_lowered_depth,
        );
        let mut output = Vec::with_capacity(arguments.len());
        for (parameter_index, (_, argument_index)) in bound.iter().enumerate() {
            // Required-only binding visits every parameter; reborrow the original
            // signature rather than returning the short-lived alignment view's borrow.
            let parameter = &parameters[parameter_index];
            let argument = &arguments[argument_index];
            budget
                .check_pending(0, arguments.len() - parameter_index)
                .map_err(|error| HelperSourceError::Output {
                    argument_index,
                    error: PureTypeError::Structure(error),
                })?;
            let (value, scalar_type) =
                lower(argument).map_err(|error| HelperSourceError::Lowering {
                    argument_index,
                    error,
                })?;
            preflight_with_budget(&value, 0, &mut budget).map_err(|error| match error {
                PureTypeError::Impure => HelperSourceError::Argument {
                    parameter_index,
                    argument_index,
                    error: ArgumentTypeError::Impure,
                },
                _ => HelperSourceError::Output {
                    argument_index,
                    error,
                },
            })?;
            let facts = ScalarArgumentType {
                scalar_type,
                is_pure: true,
                literal: match &value {
                    Node::Literal { value } => Some(value),
                    _ => None,
                },
            };
            check_argument_type(&ScalarTypeSet::only(parameter.domain), facts).map_err(
                |error| HelperSourceError::Argument {
                    parameter_index,
                    argument_index,
                    error,
                },
            )?;
            output.push(LoweredHelperArgument {
                parameter,
                argument,
                parameter_index,
                argument_index,
                value,
            });
        }
        Ok(output)
    }
}
mod frozen_call {
    use super::*;
    use crate::pure_typing::{PureTypeError, preflight_with_budget};
    use crate::source_call::valid_limits;
    type FrozenRows<'source, Node, Error> =
        Result<Vec<LoweredSourceArgument<'source, Node>>, SourceCallError<Error>>;
    pub(super) fn lower<
        'source,
        'schema,
        Key,
        Domain,
        Result,
        Capability,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Error,
    >(
        arguments: &'source [NamedArgument],
        schema: &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
        limits: SourceCallLimits,
        mut lower: impl FnMut(
            &'source NamedArgument,
        ) -> std::result::Result<
            (
                Computation<Field, Operation, HostEffect, IrResult>,
                Option<ScalarType>,
            ),
            Error,
        >,
    ) -> FrozenRows<'source, Computation<Field, Operation, HostEffect, IrResult>, Error>
    where
        Domain: ScalarArgumentDomain,
    {
        if !valid_limits(limits) {
            return Err(SourceCallError::InvalidLimits);
        }
        if schema.parameters.len() > limits.max_arguments {
            return Err(SourceCallError::ParameterLimit);
        }
        if arguments.len() > limits.max_arguments || arguments.len() > schema.parameters.len() {
            return Err(SourceCallError::ArgumentLimit);
        }
        let names = arguments
            .iter()
            .map(|argument| argument.name.as_str())
            .collect::<Vec<_>>();
        let bound = schema
            .bind_arguments(&names)
            .map_err(SourceCallError::Names)?;
        let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
        budget.visit(0, 0, 0).map_err(SourceCallError::OutputRoot)?;
        let mut lowered = Vec::with_capacity(arguments.len());
        for (parameter, argument_index) in bound.iter() {
            let argument = &arguments[argument_index];
            let parameter_index = schema
                .parameters
                .iter()
                .position(|candidate| std::ptr::eq(candidate, parameter))
                .ok_or(SourceCallError::Names(
                    NamedArgumentError::UnknownArgument {
                        index: argument_index,
                    },
                ))?;
            budget
                .check_pending(0, 1)
                .map_err(|error| SourceCallError::Output {
                    argument_index,
                    error: PureTypeError::Structure(error),
                })?;
            let (value, scalar_type) =
                lower(argument).map_err(|error| SourceCallError::Lowering {
                    argument_index,
                    error,
                })?;
            preflight_with_budget(&value, 1, &mut budget).map_err(|error| match error {
                PureTypeError::Impure => SourceCallError::Argument {
                    parameter_index,
                    argument_index,
                    error: ArgumentTypeError::Impure,
                },
                _ => SourceCallError::Output {
                    argument_index,
                    error,
                },
            })?;
            // Generated-IR preflight already proved physical purity; do not walk it again.
            let facts = ScalarArgumentType {
                scalar_type,
                is_pure: true,
                literal: match &value {
                    Computation::Literal { value } => Some(value),
                    _ => None,
                },
            };
            check_argument_type(&parameter.domain, facts).map_err(|error| {
                SourceCallError::Argument {
                    parameter_index,
                    argument_index,
                    error,
                }
            })?;
            lowered.push(LoweredSourceArgument {
                name: argument.name.as_str(),
                parameter_index,
                argument_index,
                value,
            });
        }
        // Observe the original rows without constructing the module-private wrapper.
        Ok(lowered)
    }
}

type Data = Computation<(), (), (), ()>;
const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 128,
    max_source_depth: 16,
    max_lowered_nodes: 128,
    max_lowered_depth: 16,
    max_arguments: 4,
};
fn arguments(names: &[&str]) -> Vec<NamedArgument> {
    names
        .iter()
        .enumerate()
        .map(|(i, name)| NamedArgument {
            name: (*name).into(),
            value: Expression::Integer {
                value: i as u64,
                span: Span {
                    start: i,
                    end: i + 1,
                },
            },
            span: Span {
                start: i,
                end: i + 1,
            },
        })
        .collect()
}
fn operand(
    argument: &NamedArgument,
    mode: usize,
    events: &RefCell<Vec<String>>,
) -> Result<(Data, Option<ScalarType>), &'static str> {
    events.borrow_mut().push(format!("lower:{}", argument.name));
    let integer = || Data::Literal {
        value: ScalarValue::Integer(7),
    };
    match mode {
        0 => Ok((integer(), Some(ScalarType::Integer))),
        1 => Ok((
            Data::Literal {
                value: ScalarValue::Boolean(true),
            },
            Some(ScalarType::Boolean),
        )),
        2 => Ok((
            Data::Host {
                effect: Box::new(()),
            },
            Some(ScalarType::Integer),
        )),
        3 => Ok((
            Data::Local {
                name: "input".into(),
            },
            Some(ScalarType::Integer),
        )),
        4 => Ok((
            Data::Strings {
                items: vec![integer(), integer()],
            },
            Some(ScalarType::StringList),
        )),
        5 => Err("native-refusal"),
        6 if argument.name == "b" => Err("later-refusal"),
        7 if argument.name == "b" => panic!("later-unwind"),
        _ => Ok((integer(), None)),
    }
}
fn helper_result(result: HelperSourceResult<'_, '_, Data, &'static str>) -> String {
    match result {
        Ok(rows) => format!(
            "ok:{:?}",
            rows.iter()
                .map(|r| (
                    r.parameter_index,
                    r.argument_index,
                    r.parameter.name,
                    &r.value
                ))
                .collect::<Vec<_>>()
        ),
        Err(HelperSourceError::Names(e)) => format!("names:{e:?}"),
        Err(HelperSourceError::Lowering {
            argument_index,
            error,
        }) => format!("lower:{argument_index}:{error}"),
        Err(HelperSourceError::Output {
            argument_index,
            error,
        }) => format!("output:{argument_index}:{error:?}"),
        Err(HelperSourceError::Argument {
            parameter_index,
            argument_index,
            error,
        }) => format!("argument:{parameter_index}:{argument_index}:{error:?}"),
        Err(HelperSourceError::InvalidParameter { index }) => format!("parameter:{index}"),
        Err(e) => format!("{e:?}"),
    }
}
struct Domain<'a> {
    events: &'a RefCell<Vec<String>>,
    reject: bool,
}
impl ScalarArgumentDomain for Domain<'_> {
    fn scalar_types(&self) -> ScalarTypeSet {
        self.events.borrow_mut().push("types".into());
        ScalarTypeSet::only(ScalarType::Integer)
    }
    fn accepts_literal(&self, _: &ScalarValue) -> bool {
        self.events.borrow_mut().push("literal".into());
        !self.reject
    }
}
fn call_result(
    result: Result<&[LoweredSourceArgument<'_, Data>], &SourceCallError<&'static str>>,
) -> String {
    match result {
        Ok(rows) => format!(
            "ok:{:?}",
            rows.iter()
                .map(|r| (r.parameter_index, r.argument_index, r.name, &r.value))
                .collect::<Vec<_>>()
        ),
        Err(SourceCallError::Names(e)) => format!("names:{e:?}"),
        Err(SourceCallError::OutputRoot(e)) => format!("root:{e:?}"),
        Err(SourceCallError::Lowering {
            argument_index,
            error,
        }) => format!("lower:{argument_index}:{error}"),
        Err(SourceCallError::Output {
            argument_index,
            error,
        }) => format!("output:{argument_index}:{error:?}"),
        Err(SourceCallError::Argument {
            parameter_index,
            argument_index,
            error,
        }) => format!("argument:{parameter_index}:{argument_index}:{error:?}"),
        Err(e) => format!("{e:?}"),
    }
}
const NAMES: [&[&str]; 7] = [
    &[],
    &["a"],
    &["b", "a"],
    &["a", "b"],
    &["a", "a"],
    &["x", "b"],
    &["a", "b", "x"],
];
const BUDGETS: [(usize, usize); 6] = [(0, 0), (1, 0), (2, 1), (3, 1), (8, 4), (16_385, 65)];

#[test]
fn helper_staging_matches_frozen_order_fault_positions_and_output_for_1344_cases() {
    let mut cases = 0;
    for names in NAMES {
        let args = arguments(names);
        for signature in 0..4 {
            let parameters = [
                NamedParameter::required(
                    if signature == 3 { "" } else { "a" },
                    ScalarType::Integer,
                ),
                NamedParameter {
                    name: if signature == 2 { "a" } else { "b" },
                    domain: ScalarType::Integer,
                    required: signature != 1,
                },
            ];
            for (nodes, depth) in BUDGETS {
                let limits = HelperSourceLimits {
                    source: SourceCallLimits {
                        max_lowered_nodes: nodes,
                        max_lowered_depth: depth,
                        ..LIMITS
                    },
                    max_parameters: 4,
                };
                for mode in 0..8 {
                    let run = |frozen| {
                        let events = RefCell::new(vec![]);
                        let result = catch_unwind(AssertUnwindSafe(|| {
                            if frozen {
                                helper_result(frozen_helper::lower(
                                    &args,
                                    &parameters,
                                    limits,
                                    |arg| operand(arg, mode, &events),
                                ))
                            } else {
                                helper_result(
                                    bind_helper_source_arguments(&args, &parameters, limits)
                                        .and_then(|bound| {
                                            bound.lower(|arg| operand(arg, mode, &events))
                                        }),
                                )
                            }
                        }))
                        .unwrap_or_else(|_| "unwind".into());
                        (result, events.into_inner())
                    };
                    assert_eq!(
                        run(false),
                        run(true),
                        "{names:?}, {signature}, {nodes}, {depth}, {mode}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 1344);
}

#[test]
fn host_staging_matches_frozen_domains_optional_omission_and_faults_for_2688_cases() {
    let mut cases = 0;
    for names in NAMES {
        let args = arguments(names);
        for signature in 0..4 {
            for (nodes, depth) in BUDGETS {
                let limits = SourceCallLimits {
                    max_lowered_nodes: nodes,
                    max_lowered_depth: depth,
                    ..LIMITS
                };
                for mode in 0..8 {
                    for reject in [false, true] {
                        let run = |frozen| {
                            let events = RefCell::new(vec![]);
                            let parameters = [
                                NamedParameter {
                                    name: if signature == 3 { "" } else { "a" },
                                    domain: Domain {
                                        events: &events,
                                        reject,
                                    },
                                    required: true,
                                },
                                NamedParameter {
                                    name: if signature == 2 { "a" } else { "b" },
                                    domain: Domain {
                                        events: &events,
                                        reject,
                                    },
                                    required: signature != 1,
                                },
                            ];
                            let schema = SourceSchema {
                                key: (),
                                parameters: &parameters,
                                result: (),
                                required_capability: (),
                            };
                            let result = catch_unwind(AssertUnwindSafe(|| {
                                if frozen {
                                    call_result(
                                        frozen_call::lower(&args, &schema, limits, |arg| {
                                            operand(arg, mode, &events)
                                        })
                                        .as_deref(),
                                    )
                                } else {
                                    call_result(
                                        bind_source_arguments(&args, &schema, limits)
                                            .and_then(|bound| {
                                                bound.lower(|arg| operand(arg, mode, &events))
                                            })
                                            .as_ref()
                                            .map(|call| call.arguments()),
                                    )
                                }
                            }))
                            .unwrap_or_else(|_| "unwind".into());
                            (result, events.into_inner())
                        };
                        assert_eq!(
                            run(false),
                            run(true),
                            "{names:?}, {signature}, {nodes}, {depth}, {mode}, {reject}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 2688);
}
