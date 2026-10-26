use leselang_hir as facade;
use leselang_hir_core as owner;
use leselang_runtime_core::{ScalarType, ScalarValue};
use leselang_syntax::parse;

type NativeIr = owner::ir::Computation<(), (), (), ()>;
type CompatibleIr = facade::ir::Computation<(), (), (), ()>;

#[test]
fn facade_moves_the_original_owned_ir_without_conversion_or_copy() {
    let text = String::from("original native buffer");
    let address = text.as_ptr();
    let value: CompatibleIr = CompatibleIr::Literal {
        value: ScalarValue::String(text),
    };
    let native: NativeIr = value;
    let CompatibleIr::Literal {
        value: ScalarValue::String(text),
    } = native
    else {
        panic!("literal changed across the compatibility path");
    };
    assert_eq!(text.as_ptr(), address);
}

#[test]
fn source_plans_and_diagnostics_keep_one_identity_across_both_packages() {
    let tree = parse("fn main() = 1");
    let expression = &tree.function.as_ref().unwrap().body;
    let form: owner::scalar_form::BoundScalarForm<'_> =
        facade::scalar_form::bind_scalar_form(expression).unwrap();
    let result: Result<(CompatibleIr, ScalarType), facade::scalar_source::ScalarSourceError<()>> =
        form.lower(|_| unreachable!("literal has no children"));
    assert_eq!(result.unwrap().1, ScalarType::Integer);
    let error: owner::pure_typing::PureTypeError = facade::pure_typing::PureTypeError::InvalidName;
    assert_eq!(error, owner::pure_typing::PureTypeError::InvalidName);
}

#[test]
fn native_borrowed_slots_do_not_gain_clone_send_or_serde_requirements() {
    struct LocalOnly<'a>(&'a std::cell::Cell<u8>);
    let state = std::cell::Cell::new(7);
    let compatible: facade::ir::Computation<(), (), LocalOnly<'_>, ()> =
        facade::ir::Computation::Host {
            effect: Box::new(LocalOnly(&state)),
        };
    let native: owner::ir::Computation<(), (), LocalOnly<'_>, ()> = compatible;
    let owner::ir::Computation::Host { effect } = native else {
        unreachable!()
    };
    assert!(std::ptr::eq(effect.0, &state));
}

#[cfg(feature = "leserpent-profile")]
#[test]
fn product_specialization_uses_the_external_core_with_unchanged_profile_methods() {
    let program = facade::lower(&parse(
        "fn main() = bind(x: 1, body: add(left: x, right: 2))",
    ))
    .unwrap();
    let facade::Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computation")
    };
    let native: &owner::ir::Computation<
        facade::result_field::ResultField,
        facade::host_call::HostOperation,
        facade::Effect,
        facade::Type,
    > = expression;
    native.validate_structure().unwrap();
    assert_eq!(
        native.validate_in_scope(&[]),
        Ok(facade::Type::Scalar(ScalarType::Integer))
    );
}
