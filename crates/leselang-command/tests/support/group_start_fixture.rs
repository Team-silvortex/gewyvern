use leselang_command::ResultBinding;
use leselang_runtime_core::ScalarValue;
use leserpent_hir::{Effect, computation::Computation};

pub fn binding(kind: usize) -> Option<Box<ResultBinding>> {
    if kind == 0 {
        return None;
    }
    let scalar = || Computation::Literal {
        value: ScalarValue::None,
    };
    let host = || Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let capture = |name: &str, body| Computation::Bind {
        name: name.into(),
        value: Box::new(host()),
        body: Box::new(body),
    };
    let body = match kind {
        1 => scalar(),
        2 => host(),
        3 => capture("x", scalar()),
        4 => capture("x", capture("y", scalar())),
        5 => Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(capture("x", scalar())),
            otherwise: Box::new(scalar()),
        },
        _ => unreachable!(),
    };
    Some(Box::new(ResultBinding {
        name: "group".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body,
    }))
}
