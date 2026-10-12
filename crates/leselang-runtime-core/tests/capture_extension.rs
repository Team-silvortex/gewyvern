use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_runtime_core::{
    CaptureExtensionBindings, CaptureExtensionDomain, CaptureExtensionError, CaptureExtensionSite,
    CurrentCapture, validate_capture_extension,
};

// Opaque GUI-local payloads intentionally have no Clone, Debug, Eq or wire traits.
struct Binding {
    name: &'static str,
    data: u8,
    _local: Rc<()>,
}
fn binding(name: &'static str, data: u8) -> Binding {
    Binding {
        name,
        data,
        _local: Rc::new(()),
    }
}
struct NativeFault {
    secret: Box<str>,
}
struct Domain {
    events: Rc<RefCell<Vec<String>>>,
    released: Rc<Cell<usize>>,
    fail_prepare: bool,
    panic_cleanup: bool,
}
impl Drop for Domain {
    fn drop(&mut self) {
        self.released.set(self.released.get() + 1);
        assert!(!self.panic_cleanup, "native cleanup panic");
    }
}
impl Domain {
    fn observe(&self, event: &str, old: &Binding, saved: &Binding) {
        self.events
            .borrow_mut()
            .push(format!("{event}:{}:{}", old.name, saved.name));
    }
}
impl CaptureExtensionDomain<Binding, Binding> for Domain {
    type Error = NativeFault;
    fn same_group_data(&mut self, old: &Binding, saved: &Binding) -> bool {
        self.observe("group-data", old, saved);
        old.data == saved.data
    }
    fn same_group_binding(&mut self, old: &Binding, saved: &Binding) -> bool {
        self.observe("group-binding", old, saved);
        old.name == saved.name && old.data == saved.data
    }
    fn begin_results(&mut self) -> Result<(), NativeFault> {
        self.events.borrow_mut().push("begin".into());
        if self.fail_prepare {
            Err(NativeFault {
                secret: "prepare-secret".into(),
            })
        } else {
            Ok(())
        }
    }
    fn current_result(&mut self, saved: &Binding) -> Result<CurrentCapture, NativeFault> {
        self.events
            .borrow_mut()
            .push(format!("current:{}", saved.name));
        assert_ne!(saved.data, 254, "native projection panic");
        if saved.data == 255 {
            return Err(NativeFault {
                secret: "projection-secret".into(),
            });
        }
        Ok(CurrentCapture {
            matches_result: saved.data == 2,
            matches_binding: saved.name == "now",
        })
    }
    fn same_result_data(&mut self, old: &Binding, saved: &Binding) -> bool {
        self.observe("result-data", old, saved);
        old.data == saved.data
    }
    fn same_result_binding(&mut self, old: &Binding, saved: &Binding) -> bool {
        self.observe("result-binding", old, saved);
        old.name == saved.name && old.data == saved.data
    }
}
struct Fixture {
    old_groups: Vec<Binding>,
    old_results: Vec<Binding>,
    groups: Vec<Binding>,
    results: Vec<Binding>,
    events: Rc<RefCell<Vec<String>>>,
    released: Rc<Cell<usize>>,
}
impl Fixture {
    fn new() -> Self {
        Self {
            old_groups: vec![binding("group", 10)],
            old_results: vec![binding("first", 1)],
            groups: vec![binding("group", 10), binding("group-alias", 10)],
            results: vec![
                binding("first", 1),
                binding("old-alias", 1),
                binding("now", 2),
                binding("current-alias", 2),
            ],
            events: Rc::new(RefCell::new(vec![])),
            released: Rc::new(Cell::new(0)),
        }
    }
    fn domain(&self) -> Domain {
        Domain {
            events: self.events.clone(),
            released: self.released.clone(),
            fail_prepare: false,
            panic_cleanup: false,
        }
    }
    fn run_with(
        &self,
        max: usize,
        domain: Domain,
    ) -> Result<(), CaptureExtensionError<NativeFault>> {
        validate_capture_extension(
            CaptureExtensionBindings {
                groups: &self.old_groups,
                results: &self.old_results,
            },
            CaptureExtensionBindings {
                groups: &self.groups,
                results: &self.results,
            },
            max,
            domain,
        )
    }
    fn run(&self) -> Result<(), CaptureExtensionError<NativeFault>> {
        self.run_with(8, self.domain())
    }
}

#[test]
fn borrowed_gui_aliases_keep_exact_binding_retention_and_short_circuit_order() {
    let f = Fixture::new();
    f.run().unwrap();
    assert_eq!(f.released.get(), 1);
    assert_eq!(
        &*f.events.borrow(),
        &[
            "group-data:group:group",
            "group-data:group:group-alias",
            "group-binding:group:group",
            "begin",
            "current:first",
            "result-data:first:first",
            "current:old-alias",
            "result-data:first:old-alias",
            "current:now",
            "current:current-alias",
            "result-binding:first:first",
        ]
    );
    assert_eq!(f.results[0].name, "first");
    assert_eq!(Rc::strong_count(&f.results[0]._local), 1);
}

#[test]
fn invented_group_refuses_before_retention_or_result_preparation() {
    let mut f = Fixture::new();
    f.groups[1].data = 11;
    assert!(matches!(
        f.run(),
        Err(CaptureExtensionError::UnknownGroup { index: 1 })
    ));
    assert_eq!(f.events.borrow().len(), 2);
    assert_eq!(f.released.get(), 1);
}

#[test]
fn group_alias_does_not_replace_the_original_named_binding() {
    let mut f = Fixture::new();
    f.groups.remove(0);
    assert!(matches!(
        f.run(),
        Err(CaptureExtensionError::DroppedGroup { index: 0 })
    ));
    assert!(!f.events.borrow().iter().any(|event| event == "begin"));
}

#[test]
fn non_current_saved_result_must_have_original_data() {
    let mut f = Fixture::new();
    f.results.push(binding("invented", 3));
    assert!(matches!(
        f.run(),
        Err(CaptureExtensionError::UnknownResult { index: 4 })
    ));
    assert!(
        !f.events
            .borrow()
            .iter()
            .any(|event| event.starts_with("result-binding"))
    );
}

#[test]
fn current_alias_alone_cannot_supply_the_pending_binding_name() {
    let mut f = Fixture::new();
    f.results.remove(2);
    assert!(matches!(
        f.run(),
        Err(CaptureExtensionError::MissingCurrent)
    ));
    assert!(
        !f.events
            .borrow()
            .iter()
            .any(|event| event.starts_with("result-binding"))
    );
}

#[test]
fn result_alias_does_not_replace_an_original_named_result() {
    let mut f = Fixture::new();
    f.results.remove(0);
    assert!(matches!(
        f.run(),
        Err(CaptureExtensionError::DroppedResult { index: 0 })
    ));
}

#[test]
fn current_result_never_shortcuts_later_native_projection_errors() {
    use std::error::Error;
    let mut f = Fixture::new();
    f.results.push(binding("private-node", 255));
    let error = f.run().unwrap_err();
    assert_eq!(f.released.get(), 1);
    assert!(error.source().is_none());
    assert!(!format!("{error:?} {error}").contains("secret"));
    assert!(!format!("{error:?}").contains("private-node"));
    let CaptureExtensionError::Native { site, error } = error else {
        panic!("native error expected")
    };
    assert_eq!(site, CaptureExtensionSite::Result(4));
    assert_eq!(&*error.secret, "projection-secret");
    assert!(
        !f.events
            .borrow()
            .iter()
            .any(|event| event.starts_with("result-binding"))
    );
}

#[test]
fn first_unknown_result_refuses_before_later_native_errors() {
    let mut f = Fixture::new();
    f.results[0].data = 3;
    f.results[1].data = 255;
    assert!(matches!(
        f.run(),
        Err(CaptureExtensionError::UnknownResult { index: 0 })
    ));
    assert!(
        !f.events
            .borrow()
            .iter()
            .any(|event| event == "current:old-alias")
    );
}

#[test]
fn cold_aggregate_limits_precede_every_native_hook_and_release_the_domain() {
    for (limit, which) in [(65, 0), (1, 1), (2, 2)] {
        let f = Fixture::new();
        let result = f.run_with(limit, f.domain());
        assert!(match which {
            0 => matches!(result, Err(CaptureExtensionError::InvalidLimit)),
            1 => matches!(result, Err(CaptureExtensionError::OriginalCount)),
            _ => matches!(result, Err(CaptureExtensionError::SavedCount)),
        });
        assert!(f.events.borrow().is_empty());
        assert_eq!(f.released.get(), 1);
    }
}

#[test]
fn inclusive_ceiling_and_zero_policy_are_not_implicit_defaults() {
    let mut f = Fixture::new();
    f.results.extend((0..58).map(|_| binding("alias", 2)));
    assert_eq!(f.groups.len() + f.results.len(), 64);
    f.run_with(64, f.domain()).unwrap();
    let mut empty = Fixture::new();
    empty.old_groups.clear();
    empty.old_results.clear();
    empty.groups.clear();
    empty.results.clear();
    assert!(matches!(
        empty.run_with(0, empty.domain()),
        Err(CaptureExtensionError::MissingCurrent)
    ));
    assert_eq!(&*empty.events.borrow(), &["begin"]);
}

#[test]
fn result_preparation_error_keeps_exact_native_payload_and_site() {
    let f = Fixture::new();
    let mut domain = f.domain();
    domain.fail_prepare = true;
    let CaptureExtensionError::Native { site, error } = f.run_with(8, domain).unwrap_err() else {
        panic!("native error expected")
    };
    assert_eq!(site, CaptureExtensionSite::BeginResults);
    assert_eq!(&*error.secret, "prepare-secret");
    assert_eq!(f.released.get(), 1);
    assert!(
        !f.events
            .borrow()
            .iter()
            .any(|event| event.starts_with("current:"))
    );
}

#[test]
fn projection_unwind_stops_later_hooks_and_releases_domain_once() {
    let mut f = Fixture::new();
    f.results[0].data = 254;
    assert!(catch_unwind(AssertUnwindSafe(|| f.run())).is_err());
    assert_eq!(f.released.get(), 1);
    assert!(
        !f.events
            .borrow()
            .iter()
            .any(|event| event == "current:old-alias")
    );
}

#[test]
fn native_cleanup_finishes_before_a_success_can_be_handed_off() {
    let f = Fixture::new();
    let mut domain = f.domain();
    domain.panic_cleanup = true;
    let handed_off = Cell::new(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let result = f.run_with(8, domain);
            handed_off.set(result.is_ok());
        }))
        .is_err()
    );
    assert!(!handed_off.get());
    assert_eq!(f.released.get(), 1);
}
