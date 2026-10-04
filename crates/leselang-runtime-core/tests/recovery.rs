use std::cell::Cell;
use std::rc::Rc;

use leselang_runtime_core::{
    BinaryOperator, CalculationFailure, Fuel, ScalarError, ScalarValue, UnaryOperator,
    apply_binary, apply_unary,
};

#[test]
fn recovery_class_is_closed_over_all_seven_scalar_errors() {
    const { assert!(ScalarError::IntegerArithmetic.is_recoverable()) };
    for (error, recoverable) in [
        (ScalarError::TypeMismatch, false),
        (ScalarError::UnboundedOperand, false),
        (ScalarError::IntegerArithmetic, true),
        (ScalarError::InvalidIntegerText, true),
        (ScalarError::InvalidBooleanText, true),
        (ScalarError::StringLimit, false),
        (ScalarError::StringListLimit, false),
    ] {
        assert_eq!(error.is_recoverable(), recoverable);
        assert_eq!(
            CalculationFailure::<()>::Scalar(error).is_recoverable(),
            recoverable
        );
    }
}

#[test]
fn external_codes_and_even_scalar_error_payloads_cannot_promote_their_channel() {
    struct External {
        code: &'static str,
        message: String,
    }
    for code in [
        "LSV1401",
        "LSV1408",
        "IntegerArithmetic",
        "InvalidIntegerText",
        "",
    ] {
        let failure: CalculationFailure<External> = External {
            code,
            message: "secret".into(),
        }
        .into();
        assert!(!failure.is_recoverable());
        let CalculationFailure::External(original) = failure else {
            panic!()
        };
        assert_eq!(original.code, code);
        assert_eq!(original.message, "secret");
    }
    let failure: CalculationFailure<ScalarError> = ScalarError::IntegerArithmetic.into();
    assert!(!failure.is_recoverable());
    assert!(matches!(
        failure,
        CalculationFailure::External(ScalarError::IntegerArithmetic)
    ));
}

#[test]
fn question_mark_preserves_owned_nonclone_thread_local_errors_without_conversion() {
    struct External {
        text: String,
        marker: Rc<()>,
    }
    fn forward(error: External) -> Result<(), CalculationFailure<External>> {
        Err(error)?;
        Ok(())
    }
    let marker = Rc::new(());
    let text = String::from("original buffer");
    let pointer = text.as_ptr();
    let failure = forward(External {
        text,
        marker: Rc::clone(&marker),
    })
    .unwrap_err();
    assert!(!failure.is_recoverable());
    assert_eq!(Rc::strong_count(&marker), 2);
    let CalculationFailure::External(original) = failure else {
        panic!()
    };
    assert_eq!(original.text.as_ptr(), pointer);
    assert!(Rc::ptr_eq(&original.marker, &marker));
    drop(original);
    assert_eq!(Rc::strong_count(&marker), 1);
}

#[test]
fn metadata_debug_never_invokes_an_external_formatter_or_echoes_payloads() {
    struct External;
    impl std::fmt::Debug for External {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("private payload")
        }
    }
    let failure = CalculationFailure::External(External);
    assert_eq!(format!("{failure:?}"), "External");
    assert!(!failure.is_recoverable());
    let error = CalculationFailure::<External>::Scalar(ScalarError::InvalidIntegerText);
    assert_eq!(format!("{error:?}"), "Scalar(InvalidIntegerText)");
    assert!(error.is_recoverable());
}

#[test]
fn observing_and_moving_a_failure_neither_spends_fuel_nor_releases_its_payload() {
    struct External(Rc<Cell<usize>>);
    impl Drop for External {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let fuel = Fuel::new(7);
    let failure = CalculationFailure::External(External(Rc::clone(&drops)));
    for _ in 0..100 {
        assert!(!failure.is_recoverable());
    }
    assert_eq!(format!("{failure:?}"), "External");
    assert_eq!(fuel.remaining(), 7);
    assert_eq!(drops.get(), 0);
    let moved = failure;
    assert_eq!(drops.get(), 0);
    drop(moved);
    assert_eq!(drops.get(), 1);
}

#[test]
fn send_but_nonsync_external_errors_can_move_to_a_worker_without_a_global_lock() {
    struct External {
        text: String,
        local: Cell<u8>,
    }
    let text = String::from("original buffer");
    let pointer = text.as_ptr() as usize;
    let failure = CalculationFailure::External(External {
        text,
        local: Cell::new(1),
    });
    let returned = std::thread::spawn(move || {
        assert!(!failure.is_recoverable());
        let CalculationFailure::External(error) = failure else {
            panic!()
        };
        error.local.set(2);
        error
    })
    .join()
    .unwrap();
    assert_eq!(returned.text.as_ptr() as usize, pointer);
    assert_eq!(returned.local.get(), 2);
}

#[test]
fn actual_data_operations_preserve_typed_recoverable_and_resource_failures() {
    let arithmetic = apply_binary(
        BinaryOperator::Div,
        ScalarValue::Integer(1),
        ScalarValue::Integer(0),
    )
    .map_err(CalculationFailure::<()>::Scalar)
    .unwrap_err();
    assert!(arithmetic.is_recoverable());
    for operator in [UnaryOperator::ParseInteger, UnaryOperator::ParseBoolean] {
        let failure = apply_unary(operator, ScalarValue::String("secret".into()))
            .map_err(CalculationFailure::<()>::Scalar)
            .unwrap_err();
        assert!(failure.is_recoverable());
        assert!(!format!("{failure:?}").contains("secret"));
    }
    let bounded = apply_binary(
        BinaryOperator::Concat,
        ScalarValue::String("x".repeat(4096)),
        ScalarValue::String("x".into()),
    )
    .map_err(CalculationFailure::<()>::Scalar)
    .unwrap_err();
    assert!(!bounded.is_recoverable());
    let signature = apply_unary(UnaryOperator::Not, ScalarValue::Integer(0))
        .map_err(CalculationFailure::<()>::Scalar)
        .unwrap_err();
    assert!(!signature.is_recoverable());
}

#[test]
fn native_error_destructor_panics_propagate_without_fabricating_an_outcome() {
    struct External(Rc<Cell<bool>>);
    impl Drop for External {
        fn drop(&mut self) {
            self.0.set(true);
            panic!("native cleanup failure");
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let failure = CalculationFailure::External(External(Rc::clone(&dropped)));
    assert!(!failure.is_recoverable());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(failure)));
    assert!(result.is_err());
    assert!(dropped.get());
}
