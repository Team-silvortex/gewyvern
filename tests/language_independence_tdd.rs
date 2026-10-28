use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

#[test]
fn queued_native_example_uses_separate_owners_and_original_reply_boundaries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join("crates/leselang-vm/examples/native_counter.rs"))
        .unwrap();
    for marker in [
        "Execution::new(catalog, &node, budgets[0])",
        "Execution::new(catalog, &node, budgets[1])",
        "VecDeque::with_capacity(2)",
        "for tick in 0..64",
        "queue.len() == 2",
        "Event::Dispatch",
        "Event::Reply",
        "task.accept(receipt)",
        "HostWorkOutcome::Failed",
        "exhausted_instance_does_not_stop_or_borrow_fuel_from_its_sibling",
        "delayed_foreign_and_wrong_typed_receipts_keep_the_original_handoff",
        "queued_work_and_late_receipts_never_reopen_cancelled_or_dropped_owners",
    ] {
        assert!(text.contains(marker), "{marker}");
    }
    for forbidden in ["tokio", "async fn", "std::thread", "Mutex", "Arc<"] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    let exporter =
        std::fs::read_to_string(root.join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    for mode in [
        "--queued",
        "--queued-cancel-pending",
        "--queued-cancel-reply",
    ] {
        assert!(exporter.contains(&format!("\"{mode}\"")), "{mode}");
        assert_eq!(
            exporter.matches(&format!("lean -- {mode}\\n")).count(),
            2,
            "{mode}"
        );
    }
}

#[test]
fn runnable_native_example_uses_public_facade_without_including_test_adapters() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join("crates/leselang-vm/examples/native_counter.rs"))
        .unwrap();
    for marker in [
        "leselang_vm::native",
        "compile_catalog_program",
        "LocalVm::new",
        ".with_host_outcome(",
        ".try_accept(",
        "vm.cancel()",
        "prior effects are not rolled back",
    ] {
        assert!(text.contains(marker), "{marker}");
    }
    for forbidden in [
        "include!",
        "#[path",
        "leserpent",
        "rusqlite",
        "std::process::Command",
        "serde",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    let adapter = text.split("#[cfg(test)]").next().unwrap();
    for forbidden in [".unwrap()", ".expect(", "panic!", "unsafe {"] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let exporter =
        std::fs::read_to_string(root.join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/examples/native_counter.rs"));
    assert!(exporter.contains("untracked VM example asset"));
}

#[test]
fn native_forced_publication_tests_keep_orphan_resolution_outside_the_vm() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = std::fs::read_to_string(
        root.join("crates/leselang-vm/tests/support/native_group_checkpoint.rs"),
    )
    .unwrap();
    for marker in [
        "child.0.kill().unwrap()",
        "let status = child.0.wait().unwrap()",
        "assert_eq!(status.signal(), Some(9))",
        "killed writer cannot acknowledge cancellation",
        "Only this parent owns the directory and has reaped its sole writer",
        "Discard, never publish, the orphan",
        "assert_eq!(cases, 24)",
        "recovery never removes unresolved publication",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
    assert_eq!(
        fixture
            .matches(".require_published(\"group-ledger.json\")")
            .count(),
        2
    );
    let store =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/support/native_checkpoint.rs"))
            .unwrap();
    for marker in [
        "hook(PublicationPoint::PartialWrite)",
        "hook(PublicationPoint::FileSynced)",
        "hook(PublicationPoint::Renamed)",
        "fs::symlink_metadata(self.path.join(format!(\"{name}.pending\")))",
        "unresolved fixture publication",
    ] {
        assert!(store.contains(marker), "{marker}");
    }
}

#[test]
fn native_process_reopen_uses_the_original_test_binary_and_bounded_owned_children() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = std::fs::read_to_string(
        root.join("crates/leselang-vm/tests/support/native_group_checkpoint.rs"),
    )
    .unwrap();
    for marker in [
        "Command::new(std::env::current_exe().unwrap())",
        "[\"--exact\", WORKER, \"--nocapture\", \"--test-threads=1\"]",
        "struct ChildGuard(Child)",
        "self.0.kill()",
        "self.0.wait()",
        "Duration::from_secs(30)",
        "assert_eq!(report.pid, pid)",
        "no cancellation acknowledgement before process exit",
        "std::process::exit(73)",
        "assert_eq!(cases.len(), 14)",
        "no process-local lease crosses the wire",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
    let store =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/support/native_checkpoint.rs"))
            .unwrap();
    assert!(store.contains("cleanup: false"));
    assert!(store.contains("if self.cleanup"));
}

#[test]
fn native_cancellation_persistence_stays_host_owned_and_never_acknowledges_failed_writes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = std::fs::read_to_string(
        root.join("crates/leselang-vm/tests/support/native_group_checkpoint.rs"),
    )
    .unwrap();
    for marker in [
        "revoke before writing cancellation intent",
        "fn recorded_terminal(",
        "if ledger.cancellation_requested",
        "current.cancellation_requested || current.terminal.is_some()",
        "fault.persists_cancellation() && cancelled",
        "self.cancel_observed = true",
        "FinishKind::Cancelled",
        "Fault::CancelIntentWriteFailed",
        "Fault::CancelTerminalWriteFailed",
        "stale adapter must consult the persisted fence",
        "assert_eq!(cases.len(), 150)",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
    let store =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/support/native_checkpoint.rs"))
            .unwrap();
    for marker in [
        "create_new(true)",
        "file.sync_all()?",
        "fs::rename(&pending",
        "if fail_before_publish",
        "fs::remove_file(pending)",
    ] {
        assert!(store.contains(marker), "{marker}");
    }
}

#[test]
fn native_mixed_disk_reopen_keeps_graph_provenance_separate_from_shared_schema() {
    let fixture = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("crates/leselang-vm/tests/support/native_group_checkpoint.rs"),
    )
    .unwrap();
    for marker in [
        "struct GroupOrigin",
        "struct CaptureOrigin",
        "ledger.captures != capture_origins(&image.frames, &image.completed)?",
        "original.site != saved.site",
        "original.serial != saved.serial",
        "Rc::ptr_eq(other, other_alias)",
        "Rc::ptr_eq(prior, other)",
        "Fault::CaptureDeniedAt(group_index)",
        "Fault::CaptureRevokedAt(group_index)",
        "[[Seq, Seq], [Seq, All], [All, Seq], [All, All]]",
        "assert_eq!(cases, 400)",
        "same operation is still not this member",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
}

#[test]
fn native_captured_group_disk_reopen_reuses_original_assembly_and_shared_alias_owners() {
    let fixture = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("crates/leselang-vm/tests/support/native_group_checkpoint.rs"),
    )
    .unwrap();
    for marker in [
        "restore_group_reply(plan.node, rows, BATCH",
        "aliases share one original group",
        "one restored owner for every group alias",
        "completed group never publishes",
        "Rc::ptr_eq(prior, alias)",
        "Captured::Member { serial, reply }",
        "Fault::CaptureDenied",
        "Fault::CaptureRevoked",
        "assert_eq!(cuts.len(), 25)",
        "assert_eq!(cases, 100)",
        "for corruption in 0..12",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
}

#[test]
fn native_sequence_disk_reopen_keeps_publication_separate_from_restoration_and_acceptance() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = std::fs::read_to_string(
        root.join("crates/leselang-vm/tests/support/native_group_checkpoint.rs"),
    )
    .unwrap();
    for marker in [
        "Sequence::restore_in_scope(",
        "try_into_saved(prefix)",
        "SavedSequenceReply::Prepared(",
        "SavedSequenceReply::Awaiting",
        "SavedSequenceReply::Accepted(",
        "awaiting work must not be republished",
        "acceptance must not publish its successor",
        "Fault::PublicationDenied(index)",
        "SequenceRestoreError::Frontier",
        "malformed frontier must fail before native admission",
        "assert_eq!(sequence_cuts().len(), 10)",
        "assert_eq!(cases, 12)",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
}

#[test]
fn native_group_disk_reopen_restores_parent_and_collector_under_one_owner() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = std::fs::read_to_string(
        root.join("crates/leselang-vm/tests/support/native_group_checkpoint.rs"),
    )
    .unwrap();
    for marker in [
        "Vm::restore_pending_in_scope(",
        "Parallel::restore_in_scope(",
        "try_into_pending_binding()",
        "try_into_saved()",
        "frame_site(&root, name, body)",
        "PendingReply::new(",
        "row.call.restore(&host, &id)",
        "scope.bind(None::<Vm",
        "Fuel::new(image.remaining)",
        "root != image.ir",
        "Fault::ChildDenied",
        "Fault::ChildRevoked",
        "Fault::Cancelled",
        "old.same_scope(&scope.lease())",
        "const ORDERS: [[usize; 3]; 6]",
        "Captured::Scalar(value)",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
    let signal_assertion = r#"#[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(9));
        }"#;
    assert_eq!(fixture.matches(signal_assertion).count(), 1);
    let portable = fixture.replacen(signal_assertion, "", 1);
    for forbidden in ["leserpent", "rusqlite", "unsafe {", "#[cfg("] {
        assert!(!portable.contains(forbidden), "{forbidden}");
    }
    let export = std::fs::read_to_string(root.join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(export.contains("consumer/tests/support/native_group_checkpoint.rs"));
}

#[test]
fn native_disk_reopen_reuses_original_frames_and_host_owned_receipts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/support/native_checkpoint.rs"))
            .unwrap();
    for marker in [
        "try_into_pending_binding()",
        "frame_site(&root, name, body)",
        "LocalVm::restore_pending(",
        "PreparedCall::restore(",
        "Fuel::new(image.remaining)",
        "root != image.ir",
        "candidate != store.read(\"checkpoint.json\")",
        "ledger.settled.iter().find",
        "old.same_scope(&vm.lease())",
        "vm.try_accept(&stale, reply, &host)",
        "Policy::DeniedAtRestore, Policy::DeniedAtDispatch",
        "byte equality is not a signature",
    ] {
        assert!(fixture.contains(marker), "{marker}");
    }
    for forbidden in ["leserpent", "rusqlite", "unsafe {", "#[cfg("] {
        assert!(!fixture.contains(forbidden), "{forbidden}");
    }
    let export = std::fs::read_to_string(root.join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(export.contains("consumer/tests/support/native_checkpoint.rs"));
}

#[test]
fn native_program_boundaries_distinguish_source_expansion_and_runtime_budgets() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let pipeline =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let program = pipeline
        .split_once("mod program_boundary_acceptance {")
        .unwrap()
        .1
        .split_once("mod branch_ceiling_acceptance {")
        .unwrap()
        .0;
    for marker in [
        "[CompilerBoundary; 13]",
        "[RuntimeBoundary; 6]",
        "(23, 7)",
        "(11, 3)",
        "52",
        "24",
        "152",
        "60 + nodes",
        "balanced_sum(512)",
        "MAX_CALL_DEPTH",
        "LSE1110",
        "required - 1",
        "ceiling + 1, usize::MAX",
        "Prepared operands have their own host policy",
        "p.identity.borrow().is_none()",
    ] {
        assert!(program.contains(marker), "{marker}");
    }
}

#[test]
fn native_branch_ceiling_acceptance_uses_public_limits_without_a_product_call_budget() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let pipeline =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let ceiling = pipeline
        .split_once("mod branch_ceiling_acceptance {")
        .unwrap()
        .1
        .split_once("mod prepared_acceptance {")
        .unwrap()
        .0;
    for marker in [
        "group_exports::MAX_GROUP_EXPORT_MEMBERS",
        "assert_eq!(LIMIT, 64)",
        "LIMIT - usize::from(successor)",
        "2 * LIMIT - 1",
        "tail__m{members}",
        "first_request_cost.unwrap() - 1",
        "used - 1",
        "32",
        "64",
        "40",
        "assert_eq!(host.serial.get(), 0)",
    ] {
        assert!(ceiling.contains(marker), "{marker}");
    }
    let typing =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/flow_typing.rs")).unwrap();
    assert!(typing.contains("max_calls includes all cold call sites"));
}

#[test]
fn native_prepared_pipeline_preserves_the_preparation_before_identity_boundary() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let pipeline =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let prepared = pipeline
        .split_once("mod prepared_acceptance {")
        .unwrap()
        .1
        .split_once("mod declaration_acceptance {")
        .unwrap()
        .0;
    for marker in [
        "const SHAPES: [&str; 4]",
        "const FAULTS: [Fault; 5]",
        "192",
        "320",
        "40",
        "tail__iteration_3__right",
        "used - 1",
        "assert_eq!(host.serial.get(), 0)",
        "p.identity.borrow().is_none()",
        "p.members.borrow().is_empty()",
    ] {
        assert!(prepared.contains(marker), "{marker}");
    }
}

#[test]
fn native_declaration_pipeline_uses_the_public_declaration_and_parameter_ceilings() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let syntax = std::fs::read_to_string(root.join("crates/leselang-syntax/src/lib.rs")).unwrap();
    let pipeline =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    for (name, count) in [("MAX_FUNCTIONS", 32), ("MAX_FUNCTION_PARAMETERS", 8)] {
        assert!(syntax.contains(&format!("pub const {name}: usize = {count};")));
        assert!(pipeline.contains(&format!("leselang_vm::native::syntax::{name}")));
    }
    let declarations = pipeline
        .split_once("mod declaration_acceptance {")
        .unwrap()
        .1
        .split_once("mod control_acceptance {")
        .unwrap()
        .0;
    for marker in [
        "const ARGUMENTS: [&str; 8]",
        "arguments.reverse()",
        "arguments.rotate_left(3)",
        "432",
        "576",
        "(72, 936)",
        "used - 1",
        "unselected || recoverable_first",
    ] {
        assert!(declarations.contains(marker), "{marker}");
    }
}

#[test]
fn native_control_pipeline_covers_the_closed_scalar_type_inventory() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let values =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/value.rs")).unwrap();
    let pipeline =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let variants = values
        .split_once("pub enum ScalarType {")
        .unwrap()
        .1
        .split_once('}')
        .unwrap()
        .0
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.strip_suffix(',').unwrap())
        .collect::<BTreeSet<_>>();
    let tested = pipeline
        .split_once("const TYPES:")
        .unwrap()
        .1
        .split_once("];")
        .unwrap()
        .0
        .lines()
        .filter_map(|line| line.trim().strip_prefix("ScalarType::"))
        .map(|line| line.trim_end_matches(','))
        .collect::<BTreeSet<_>>();
    assert_eq!(variants.len(), 6);
    assert_eq!(tested, variants);
    let controls = pipeline
        .split_once("mod control_acceptance {")
        .unwrap()
        .1
        .split_once("mod scalar_acceptance {")
        .unwrap()
        .0;
    for marker in [
        "const FORMS: [&str; 8]",
        "576",
        "288",
        "648",
        "used - 1",
        "fn typed(value: {name}) = bind(caller: value, body: caller)",
    ] {
        assert!(controls.contains(marker), "{marker}");
    }
}

#[test]
fn native_scalar_pipeline_acceptance_covers_the_current_closed_operator_inventory() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let operators =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/scalar_ops.rs"))
            .unwrap();
    let pipeline =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let scalar = pipeline.split_once("mod scalar_acceptance {").unwrap().1;
    for (name, array, count) in [
        ("BinaryOperator", "BINARY", 23),
        ("UnaryOperator", "UNARY", 7),
    ] {
        let declaration = operators
            .split_once(&format!("pub enum {name} {{"))
            .unwrap()
            .1
            .split_once('}')
            .unwrap()
            .0;
        let variants = declaration
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.strip_suffix(',').unwrap())
            .collect::<BTreeSet<_>>();
        let list = scalar
            .split_once(&format!("const {array}:"))
            .unwrap()
            .1
            .split_once("];")
            .unwrap()
            .0;
        let actual = list
            .lines()
            .filter_map(|line| line.trim().strip_prefix(&format!("{name}::")))
            .map(|line| line.trim_end_matches(','))
            .collect::<BTreeSet<_>>();
        assert_eq!(variants.len(), count);
        assert_eq!(actual, variants, "{name} gained an uncovered variant");
    }
    for marker in [
        "assert_eq!(probes.len(), 44)",
        "assert_eq!(valid.len(), 30)",
        "528",
        "1440",
        "used - 1",
        "expected.clone()",
    ] {
        assert!(scalar.contains(marker), "{marker}");
    }
    let oracle = scalar.split_once("fn pipeline<").unwrap().0;
    assert!(!oracle.contains("apply_binary("));
    assert!(!oracle.contains("apply_unary("));
}

#[test]
fn product_hir_consumers_do_not_depend_on_language_compatibility_facades() {
    let graph = workspace_dependency_graph();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for package in [
        "leselang-command",
        "leselang-ui",
        "leselang-observe",
        "leserpent-vm",
        "leserpentd",
        "leserpent-cli",
        "leserpent-runtime",
    ] {
        let closure = dependency_closure(&graph, package);
        for facade in ["leselang-hir", "leselang-vm"] {
            assert!(
                !closure.contains(facade),
                "{package} still routes through {facade}"
            );
        }
        let base = root.join("crates").join(package);
        let manifest = std::fs::read_to_string(base.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("leserpent-hir = { path = \"../leserpent-hir\" }"));
        assert!(!manifest.contains("path = \"../leselang-hir\""));
        assert!(!manifest.contains("package = \"leselang-hir\""));
        let mut pending = vec![base];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let code = std::fs::read_to_string(&path).unwrap();
                    assert!(!code.contains("leselang_hir::"), "{}", path.display());
                    assert!(!code.contains("leselang_hir as "), "{}", path.display());
                }
            }
        }
    }
    for package in ["leselang-command", "leserpent-vm"] {
        let dependencies = &graph[package];
        assert!(dependencies.contains("leselang-hir-core"));
        assert!(dependencies.contains("leserpent-hir"));
    }
}

#[test]
fn product_hir_owner_is_one_way_and_the_compatibility_facade_contains_no_implementation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    let manifest = read("crates/leserpent-hir/Cargo.toml");
    assert!(manifest.contains("leselang-hir-core = { path = \"../leselang-hir-core\" }"));
    assert!(!manifest.contains("leselang-hir ="));
    assert!(!manifest.contains("[features]"));
    let product = read("crates/leserpent-hir/src/lib.rs");
    assert!(product.contains("pub(crate) use leselang_hir_core::*;"));
    assert!(product.contains("include!(\"leserpent_profile.rs\");"));
    let facade = read("crates/leselang-hir/src/lib.rs");
    for forbidden in ["include!(", "pub mod ", "fn lower(", "struct HirProgram"] {
        assert!(!facade.contains(forbidden), "{forbidden}");
    }
    assert!(facade.contains("#[cfg(feature = \"leserpent-profile\")]\npub use leserpent_hir::*;"));
    let mut directories = vec![root.join("crates/leselang-hir/src")];
    let mut sources = Vec::new();
    // Git does not track empty directories left behind by source moves.
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                directories.push(entry.path());
            } else {
                sources.push(entry.path());
            }
        }
    }
    assert_eq!(sources, [root.join("crates/leselang-hir/src/lib.rs")]);
    let lock = read("tests/fixtures/leselang-product-hir-boundary/Cargo.lock");
    let native = lock
        .split("[[package]]")
        .find(|p| p.contains("name = \"leselang-hir\"\n"))
        .unwrap();
    assert!(!native.contains("leserpent-hir"));
    let ci = read(".github/workflows/ci.yml");
    for command in [
        "cargo +1.98.0 test -p leserpent-hir --locked",
        "cargo +1.98.0 test -p leselang-hir --locked --test product_identity",
        "cargo +1.98.0 clippy -p leserpent-hir --locked --all-targets -- -D warnings",
    ] {
        assert!(ci.contains(command), "{command}");
    }
}

#[test]
fn native_hir_owner_is_physical_product_free_and_reexported_without_a_second_tree() {
    assert_workspace_closure(
        "leselang-hir-core",
        &["leselang-runtime-core", "leselang-syntax"],
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    let manifest = read("crates/leselang-hir-core/Cargo.toml");
    for forbidden in [
        "[features]",
        "leserpent",
        "leselang-host-contract",
        "leselang-command",
        "path = \"../leselang-hir\"",
    ] {
        assert!(!manifest.contains(forbidden), "{forbidden}");
    }
    let core = read("crates/leselang-hir-core/src/lib.rs");
    for line in core.lines() {
        let Some(module) = line
            .strip_prefix("pub mod ")
            .or_else(|| line.strip_prefix("mod "))
            .and_then(|s| s.strip_suffix(';'))
        else {
            continue;
        };
        assert!(
            root.join(format!("crates/leselang-hir-core/src/{module}.rs"))
                .is_file()
        );
        assert!(
            !root
                .join(format!("crates/leselang-hir/src/{module}.rs"))
                .exists()
        );
    }
    let facade = read("crates/leselang-hir/src/lib.rs");
    assert!(facade.contains("pub use leselang_hir_core::*;"));
    assert!(!facade.contains("pub mod ir;"));
    assert!(facade.contains("pub use leserpent_hir::*;"));
    let product = read("crates/leserpent-hir/src/computation.rs");
    assert!(!product.contains("mod scalar_form_contract_tests;"));
    let harness = read("crates/leselang-hir/tests/support/migration_computation.rs");
    assert!(harness.contains("include!(\"../../../leserpent-hir/src/computation.rs\");"));
    assert!(harness.contains("mod scalar_form_contract_tests;"));
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs").contains("\"consumer/tests/core_identity.rs\"")
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("cargo +1.98.0 test -p leselang-hir-core --locked"));
    let install = ci
        .split("      - name: Install Rust toolchain\n")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap();
    assert!(install.contains("--component rustfmt,clippy"));
}

#[test]
fn product_hir_is_compiled_as_a_foreign_crate_without_reference_feature_unification() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    let path = "tests/fixtures/leselang-product-hir-boundary/";
    let manifest = read(&format!("{path}Cargo.toml"));
    assert!(manifest.contains("[workspace]"));
    assert!(manifest.contains(
        "leselang-hir = { path = \"../../../crates/leselang-hir\", default-features = false }"
    ));
    assert!(!manifest.contains("features = [\"leserpent-profile\"]"));
    assert!(manifest.contains("publish = false"));
    let source = read(&format!("{path}src/lib.rs"));
    assert!(source.contains("pub use leserpent_hir::*;"));
    assert!(!source.contains("include!("));
    assert!(manifest.contains("leserpent-hir = { path = \"../../../crates/leserpent-hir\" }"));
    assert!(!source.contains("pub use leselang_hir::*"));
    assert!(!source.contains("fn lower("));
    let lock = read(&format!("{path}Cargo.lock"));
    let native = lock
        .split("[[package]]")
        .find(|p| p.contains("name = \"leselang-hir\"\n"))
        .unwrap();
    assert!(!native.contains("leselang-host-contract"));
    let naming = read("crates/leselang-hir-core/src/pure_typing.rs");
    assert!(naming.contains("pub fn valid_local_name(name: &str) -> bool"));
    assert!(naming.contains("pub(crate) fn valid_member_name"));
    assert!(
        read("crates/leserpent-hir/src/computation.rs")
            .contains("crate::pure_typing::valid_local_name(name)")
    );
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs").contains("\"consumer/tests/local_names.rs\"")
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--no-default-features --test local_names"));
    assert!(ci.contains("test --manifest-path tests/fixtures/leselang-product-hir-boundary/Cargo.toml --locked --target-dir target --test product"));
    assert!(ci.contains("clippy --manifest-path tests/fixtures/leselang-product-hir-boundary/Cargo.toml --locked --target-dir target --lib --test product -- -D warnings"));
}

#[test]
fn public_scalar_form_removes_product_private_scalar_kernel_access_without_weakening_strict_ingress()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p| std::fs::read_to_string(root.join(p)).unwrap();
    let source = read("crates/leselang-hir-core/src/scalar_form.rs");
    for marker in [
        "pub struct BoundScalarForm",
        "pub fn bind_scalar_form",
        "primitive(expression).then_some",
        "lower_form(self.expression",
        "pure: value.is_pure()",
        "ScalarSourceError::Native",
    ] {
        assert!(source.contains(marker), "{marker}");
    }
    for forbidden in [
        "pub expression:",
        "pub pure:",
        "apply_binary(",
        "apply_unary(",
        "leserpent_contract",
        "derive(Serialize",
        "derive(Deserialize",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    let adapter = read("crates/leserpent-hir/src/computation.rs");
    assert!(adapter.contains("crate::scalar_form::bind_scalar_form(expression)"));
    for line in adapter
        .lines()
        .filter(|l| l.contains("crate::scalar_source::"))
    {
        assert!(
            line.contains("ScalarSourceError"),
            "product scalar private coupling: {line}"
        );
    }
    let kernel = read("crates/leselang-hir-core/src/scalar_source.rs");
    assert!(kernel.contains("pub(crate) fn lower_form"));
    assert!(kernel.contains("preflight(expression, limits)"));
    assert!(kernel.contains("preflight_names(expression)"));
    let frozen = read("crates/leserpent-hir/src/scalar_form_contract_tests.rs");
    let oracle = frozen.split_once("#[test]").unwrap().0;
    assert!(oracle.contains("crate::scalar_source::lower_form"));
    assert!(!oracle.contains("bind_scalar_form"));
    assert!(frozen.contains("assert_eq!(cases, 3840)"));
    let consumer = read("crates/leselang-hir/tests/scalar_form.rs");
    assert!(consumer.contains("assert_eq!(cases, 870)"));
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs").contains("\"consumer/tests/scalar_form.rs\"")
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--no-default-features --test scalar_form"));
    assert!(ci.contains("--test migration_oracles computation::scalar_form_contract_tests"));
}

#[test]
fn public_fold_stages_keep_collection_first_and_next_admission_after_scope_cleanup() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p| std::fs::read_to_string(root.join(p)).unwrap();
    let source = read("crates/leselang-hir-core/src/fold_source.rs");
    for marker in [
        "pub struct BoundFoldSource",
        "pub struct FoldItemsSource",
        "pub struct PreparedFoldSource",
        "pub struct LoweredFoldSource",
        "pub fn bind_fold_source",
        "let limit = self.source.limit()?",
        "pub fn prepare_items",
        "pub fn prepare_initial",
        "pub fn lower_next",
        "pub fn finish",
        "pure: value.is_pure()",
        "pure: next.is_pure()",
    ] {
        assert!(source.contains(marker), "{marker}");
    }
    for forbidden in [
        "pub pure:",
        "pub source:",
        "pub initial:",
        "pub next:",
        "pub limit:",
        "FoldCursor",
        "0..limit",
        "leserpent_contract",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    let next = source
        .split_once("pub fn lower_next")
        .unwrap()
        .1
        .split_once("pub fn finish")
        .unwrap()
        .0;
    assert!(!next.contains("fold_next("));
    assert!(!next.contains(".is_pure()"));
    let adapter = read("crates/leserpent-hir/src/computation.rs");
    let lower = adapter
        .split_once("fn lower_fold")
        .unwrap()
        .1
        .split_once("pub(super) fn valid_local")
        .unwrap()
        .0;
    for forbidden in [
        "scalar_source::fold_source",
        "scalar_source::fold_items",
        "scalar_source::scalar_operand",
        "scalar_source::fold_next",
        "Operand {",
    ] {
        assert!(!lower.contains(forbidden), "{forbidden}");
    }
    assert!(lower.contains("scope.nested()"));
    assert!(lower.contains(".check_scope(scope, None)"));
    assert!(lower.find(".prepare_items(").unwrap() < lower.find(".prepare_initial(").unwrap());
    let frozen = read("crates/leserpent-hir/src/fold_preparation_contract_tests.rs");
    let oracle = frozen.split_once("#[test]").unwrap().0;
    assert!(oracle.contains("crate::scalar_source::fold_source"));
    assert!(!oracle.contains("bind_fold_source"));
    assert!(frozen.contains("assert_eq!(cases, 1920)"));
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs")
            .contains("\"consumer/tests/fold_preparation.rs\"")
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--no-default-features --test fold_preparation"));
    assert!(ci.contains("--test migration_oracles computation::fold_preparation_contract_tests"));
}

#[test]
fn public_loop_stages_replace_private_product_gates_without_exposing_unchecked_purity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p| std::fs::read_to_string(root.join(p)).unwrap();
    let source = read("crates/leselang-hir-core/src/loop_source.rs");
    for marker in [
        "pub struct BoundLoopSource",
        "pub struct PreparedLoopSource",
        "pub struct LoweredLoopSource",
        "pub fn bind_loop_source",
        "let limit = self.source.limit()?",
        "pure: value.is_pure()",
        "lower_loop_body(&self.source, self.state_type",
        "pub fn finish(self)",
    ] {
        assert!(source.contains(marker), "{marker}");
    }
    for forbidden in [
        "pub pure:",
        "pub source:",
        "pub initial:",
        "pub body:",
        "pub limit:",
        "LoopBudget::new",
        "0..limit",
        "leserpent_contract",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    let adapter = read("crates/leserpent-hir/src/computation.rs");
    let lower = adapter
        .split_once("fn lower_loop")
        .unwrap()
        .1
        .split_once("fn lower_fold")
        .unwrap()
        .0;
    for forbidden in [
        "scalar_source::loop_source",
        "scalar_source::lower_loop_body",
        "scalar_source::scalar_operand",
        "Operand {",
    ] {
        assert!(!lower.contains(forbidden), "{forbidden}");
    }
    assert!(lower.contains("scope.nested()"));
    assert!(lower.contains("Ok(lowered.finish())"));
    let frozen = read("crates/leserpent-hir/src/loop_preparation_contract_tests.rs");
    let oracle = frozen.split_once("#[test]").unwrap().0;
    assert!(oracle.contains("crate::scalar_source::loop_source"));
    assert!(!oracle.contains("bind_loop_source"));
    assert!(frozen.contains("assert_eq!(cases, 1152)"));
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs")
            .contains("\"consumer/tests/loop_preparation.rs\"")
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--no-default-features --test loop_preparation"));
    assert!(ci.contains("--test migration_oracles computation::loop_preparation_contract_tests"));
}

#[test]
fn public_native_source_plans_keep_original_borrows_and_do_not_replace_strict_ingress() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p| std::fs::read_to_string(root.join(p)).unwrap();
    let host = read("crates/leselang-hir-core/src/host_source.rs");
    assert!(host.contains("pub struct BoundHostSource<'source>"));
    assert!(host.contains("pub fn bind_host_source_root"));
    assert!(!host.contains("lower_preflighted_host_source"));
    let fields = host
        .split_once("pub struct BoundHostSource")
        .unwrap()
        .1
        .split_once("impl fmt")
        .unwrap()
        .0;
    assert!(!fields.contains("pub expression:"));
    assert!(!fields.contains("pub at:"));
    let bind = host
        .split_once("pub fn bind_host_source_root")
        .unwrap()
        .1
        .split_once("impl<'source>")
        .unwrap()
        .0;
    assert!(bind.contains(".visit(0, 0, 0)"));
    assert!(!bind.contains("prepare(expression)"));
    assert!(!bind.contains("preflight(expression"));
    let field = read("crates/leselang-hir-core/src/projection_source.rs");
    assert!(field.contains("pub struct BoundFieldSource<'source>(FieldSource<'source>)"));
    assert!(field.contains("pub fn bind_field_source"));
    assert!(field.contains("self.0.lower_preflighted(limits, lower, decode, export)"));
    assert!(field.contains("preflight(expression, limits)"));
    assert!(field.contains("preflight_names(expression)?"));
    let adapter = read("crates/leserpent-hir/src/computation.rs");
    assert!(adapter.contains("crate::projection_source::bind_field_source("));
    assert!(adapter.contains("crate::host_source::bind_host_source_root("));
    assert!(!adapter.contains("crate::projection_source::field_source("));
    assert!(!adapter.contains("lower_preflighted_host_source"));
    let frozen = read("crates/leselang-hir-core/src/source_preparation_contract_tests.rs");
    assert!(frozen.contains("assert_eq!(cases, 864)"));
    assert!(frozen.contains("assert_eq!(cases, 144)"));
    let oracle = frozen.split_once("type Data =").unwrap().0;
    assert!(!oracle.contains("bind_field_source("));
    assert!(!oracle.contains("bind_host_source_root("));
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs")
            .contains("\"consumer/tests/source_preparation.rs\"")
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--no-default-features --test source_preparation"));
    assert!(ci.contains("--lib source_preparation_contract_tests"));
}

#[test]
fn public_argument_stages_are_bounded_named_alignments_not_source_or_authority_receipts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p| std::fs::read_to_string(root.join(p)).unwrap();
    for (path, name, binder, old) in [
        (
            "crates/leselang-hir-core/src/helper_source.rs",
            "BoundHelperSourceArguments",
            "bind_helper_source_arguments",
            "lower_preflighted_helper_arguments",
        ),
        (
            "crates/leselang-hir-core/src/source_call.rs",
            "BoundSourceArguments",
            "bind_source_arguments",
            "lower_preflighted_arguments",
        ),
    ] {
        let source = read(path);
        assert!(source.contains(&format!("pub struct {name}")));
        assert!(source.contains(&format!("pub fn {binder}")));
        assert!(!source.contains(old));
        let state = source
            .split_once(&format!("pub struct {name}"))
            .unwrap()
            .1
            .split_once("impl")
            .unwrap()
            .0;
        for field in ["arguments", "parameters", "schema", "bindings", "limits"] {
            assert!(!state.contains(&format!("pub {field}:")));
        }
        let binding = source
            .split_once(&format!("pub fn {binder}"))
            .unwrap()
            .1
            .split_once("impl")
            .unwrap()
            .0;
        assert!(!binding.contains("check_argument_type("));
        assert!(!binding.contains("preflight_with_budget("));
        assert!(!binding.contains("lower(argument)"));
        assert!(source.contains("pub fn lower<"));
    }
    let helper = read("crates/leserpent-hir/src/functions.rs");
    let host = read("crates/leserpent-hir/src/computation.rs");
    assert!(helper.contains("cached.revalidate(template_limits())"));
    assert!(!helper.contains("crate::helper_templates::preflight("));
    assert!(helper.contains("bind_helper_source_arguments("));
    assert!(host.contains("bind_source_arguments("));
    assert!(!helper.contains("lower_preflighted_helper_arguments"));
    assert!(!host.contains("lower_preflighted_arguments"));
    let frozen = read("crates/leselang-hir-core/src/argument_binding_contract_tests.rs");
    assert!(frozen.contains("assert_eq!(cases, 1344)"));
    assert!(frozen.contains("assert_eq!(cases, 2688)"));
    let oracle = frozen.split_once("type Data =").unwrap().0;
    assert!(!oracle.contains("bind_helper_source_arguments("));
    assert!(!oracle.contains("bind_source_arguments("));
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs")
            .contains("\"consumer/tests/argument_binding.rs\"")
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--no-default-features --test argument_binding"));
    assert!(ci.contains("--lib argument_binding_contract_tests"));
}

#[test]
fn declaration_receipts_replace_private_compiler_bypasses_without_unchecked_source_constructors() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p| std::fs::read_to_string(root.join(p)).unwrap();
    let declarations = read("crates/leselang-hir-core/src/helper_declarations.rs");
    for contract in [
        "pub struct DeclarationBody<'source>",
        "pub fn source_for(&self, function: &Function)",
        "std::ptr::eq(*original, function)",
        "max_nodes >= self.source_bounds.0 && max_depth >= self.source_bounds.1",
        "budget.check_pending",
    ] {
        assert!(
            declarations
                .replace(['\n', ' '], "")
                .contains(&contract.replace(' ', "")),
            "{contract}"
        );
    }
    let receipt = declarations
        .split_once("pub struct DeclarationBody<'source>")
        .unwrap()
        .1
        .split_once("impl fmt::Debug")
        .unwrap()
        .0;
    assert!(!receipt.contains("pub function:"));
    assert!(!receipt.contains("pub source_bounds:"));
    let body = read("crates/leselang-hir-core/src/helper_body_source.rs");
    assert!(!body.contains("lower_preflighted_helper_body"));
    assert!(!body.contains("leserpent-profile"));
    let declared = body
        .split_once("pub fn lower_declared_helper_body<")
        .unwrap()
        .1;
    assert!(declared.find("prepare_header(").unwrap() < declared.find(".check_bounds(").unwrap());
    assert!(declared.find(".check_bounds(").unwrap() < declared.find("finish(").unwrap());
    let program = read("crates/leselang-hir-core/src/program_source.rs");
    assert!(!program.contains("pub(crate) fn assemble_preflighted_program"));
    assert!(program.contains("fn assemble_preflighted_program<"));
    let declared = program
        .split_once("pub fn assemble_declared_program<")
        .unwrap()
        .1;
    assert!(
        declared.find(".check_bounds(").unwrap()
            < declared.find("assemble_preflighted_program(").unwrap()
    );
    let product = read("crates/leserpent-hir/src/functions.rs");
    assert!(product.contains("accepted.source_for(function)"));
    assert!(!product.contains("lower_preflighted_helper_body"));
    assert!(!product.contains("assemble_preflighted_program"));
    assert!(
        read("crates/gewyvern-dev/src/leselang.rs")
            .contains("\"consumer/tests/declaration_source.rs\"")
    );
    assert!(
        read(".github/workflows/ci.yml")
            .contains("--no-default-features --test declaration_source")
    );
}

#[test]
fn computation_policy_is_foreign_host_implementable_without_inherent_alias_methods() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |p| std::fs::read_to_string(root.join(p)).unwrap();
    let generic = read("crates/leselang-hir-core/src/computation_profile.rs");
    assert!(generic.contains("pub trait ComputationProfile<Field, Operation, ResultType>"));
    assert_eq!(generic.matches("pub fn ").count(), 12);
    for forbidden in [
        "leserpent",
        "HostOperation",
        "CanonicalSourceError",
        "serde::",
        ": Clone",
        ".clone(",
        ".collect(",
        "Box::",
        "Vec::",
        "Mutex",
        "catch_unwind(",
        "children()",
        "validate_shape(",
    ] {
        assert!(!generic.contains(forbidden), "{forbidden}");
    }
    let adapter = read("crates/leserpent-hir/src/computation.rs");
    assert!(!adapter.contains("impl Computation {"));
    assert!(adapter.contains("ComputationProfile<ResultField, HostOperation, Type> for Effect"));
    assert!(adapter.contains("pub(super) fn validate_in_type_scope("));
    let frozen = read("crates/leserpent-hir/src/computation_profile_contract_tests.rs");
    let oracle = frozen.split("fn scalar()").next().unwrap();
    for name in [
        "is_atomic_tail",
        "is_atomic_capture",
        "is_result_chain",
        "is_result_flow",
        "atomic_flow_bound",
        "validate_in_group_scope",
    ] {
        assert!(!oracle.contains(&format!(".{name}(")));
        assert!(oracle.contains(&format!("fn frozen_{name}(")));
    }
    assert!(frozen.contains("assert_eq!(cases, 880)"));
    assert!(frozen.contains("assert_eq!(cases, 165)"));
    let native = read("crates/leselang-hir/tests/computation_profile.rs");
    assert!(!native.contains("#![cfg(feature"));
    assert!(!native.contains("leserpent"));
    let export = read("crates/gewyvern-dev/src/leselang.rs");
    assert!(export.contains("\"consumer/tests/computation_profile.rs\""));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--no-default-features --test computation_profile"));
    assert!(ci.contains("--lib computation::profile_contract_tests"));
}

#[test]
fn owned_capture_data_lives_in_the_product_free_core_and_reference_types_are_exact_aliases() {
    assert_workspace_closure("leselang-runtime-core", &[]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/captured_bindings.rs"))
            .unwrap();
    for declaration in [
        "pub struct ScalarBinding",
        "pub struct ProjectedBinding<Result>",
        "pub struct ProjectedGroupBinding<Group>",
        "pub struct ProjectedGroup<Result>",
        "pub struct CapturedBindings<Body, Result, Group>",
    ] {
        assert!(shared.contains(declaration));
    }
    for absent in [
        "leselang_hir",
        "leserpent_domain",
        "rusqlite",
        "Revision",
        "HostOperation",
    ] {
        assert!(!shared.contains(absent));
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-vm/src/result_binding.rs")).unwrap();
    assert!(reference.contains("pub use leselang_runtime_core::ScalarBinding;"));
    let binding =
        std::fs::read_to_string(root.join("crates/leselang-command/src/result_binding.rs"))
            .unwrap();
    assert!(binding.contains(
        "leselang_runtime_core::CapturedBindings<Computation, ProjectedResult, ProjectedGroup>"
    ));
    assert!(!reference.contains("pub type ResultBinding"));
    for absent in [
        "pub struct ScalarBinding",
        "pub struct ResultBinding",
        "pub struct ProjectedBinding",
        "pub struct ProjectedGroupBinding",
        "pub struct ProjectedGroup {",
    ] {
        assert!(!reference.contains(absent));
    }
    assert!(reference.contains("impl ReferenceResultBinding for ResultBinding"));
    assert!(reference.contains("leselang_command::ProjectedGroupExt as ReferenceProjectedGroup"));
    assert!(reference.contains("leselang_command::validate_result_binding("));
    assert!(reference.contains("pub use leselang_command::{"));
    assert!(!reference.contains("pub struct ProjectedResult"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/projected_result.rs"))
            .unwrap();
    assert!(adapter.contains("validate_scalar_projection(&self.fields, expected)"));
    assert!(adapter.contains("pub struct ProjectedResult"));
    let groups =
        std::fs::read_to_string(root.join("crates/leselang-command/src/projected_group.rs"))
            .unwrap();
    for alias in [
        "pub type ProjectedBinding = leselang_runtime_core::ProjectedBinding<ProjectedResult>;",
        "pub type ProjectedGroupBinding = leselang_runtime_core::ProjectedGroupBinding<ProjectedGroup>;",
        "pub type ProjectedGroup = leselang_runtime_core::ProjectedGroup<ProjectedResult>;",
    ] {
        assert!(groups.contains(alias));
        assert!(!reference.contains(alias));
    }
}

#[test]
fn capture_data_extraction_keeps_native_ownership_and_real_memory_durable_compatibility_proofs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, proofs) in [
        (
            "crates/leselang-runtime-core/tests/captured_bindings.rs",
            &[
                "empty_optional_arrays_keep_legacy_wire_without_native_default_bounds",
                "second_unrelated_schema_uses_opaque_body_result_and_group_without_core_interpretation",
                "move_only_thread_local_native_payloads_and_text_buffers_keep_original_ownership",
                "direct_and_plain_text_decoded_data_are_not_validation_or_authority_certificates",
            ][..],
        ),
        (
            "crates/leselang-vm/tests/capture_contract.rs",
            &[
                "all_five_legacy_capture_paths_are_exact_shared_types_without_conversion",
                "legacy_empty_and_populated_capture_bytes_keep_field_order_and_projection_version_rules",
                "real_successor_capture_uses_shared_frames_and_reenters_once_with_original_fuel",
                "shared_data_never_bypasses_product_frame_validation_or_live_original_image_fences",
            ][..],
        ),
        (
            "crates/leselang-vm/tests/dataflow.rs",
            &[
                "shared_owned_capture_frames_keep_exact_journal_bytes_and_restart_successor_authority",
            ][..],
        ),
    ] {
        let code = std::fs::read_to_string(root.join(path)).unwrap();
        for proof in proofs {
            assert!(code.contains(proof), "capture extraction lost {proof}");
        }
    }
}

#[test]
fn real_reference_capture_validation_uses_product_free_helpers_without_moving_authority_policy() {
    assert_workspace_closure("leselang-runtime-core", &[]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let shared = std::fs::read_to_string(
        root.join("crates/leselang-runtime-core/src/capture_validation.rs"),
    )
    .unwrap();
    for api in [
        "pub fn validate_captured_bindings",
        "pub fn validate_projected_group",
        "pub enum CaptureStructureError",
        "HashSet::new()",
        "checked_add",
    ] {
        assert!(shared.contains(api));
    }
    for absent in [
        "leselang_hir",
        "leserpent_domain",
        "rusqlite",
        "HostOperation",
        "CapabilitySet",
        "Principal",
        "projection_version",
        "Serialize",
        "Deserialize",
    ] {
        assert!(!shared.contains(absent));
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-vm/src/result_binding.rs")).unwrap();
    for retained in [
        "leselang_command::validate_result_binding_structure(",
        "leselang_command::reference_request::result_binding_limits()",
        "crate::valid_merge_branch_name",
        ".map_err(projection_fault)",
        "leselang_command::validate_result_binding(",
    ] {
        assert!(
            reference.contains(retained),
            "capture adapter lost {retained}"
        );
    }
    let policy =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    for retained in [
        "max_saved_bindings: MAX_EFFECT_NESTING_DEPTH",
        "max_name_bytes: MAX_BRANCH_NAME_BYTES",
        "max_members: crate::MAX_MERGE_BRANCHES - 1",
    ] {
        assert!(policy.contains(retained), "capture policy lost {retained}");
        assert!(!reference.contains(retained), "VM duplicated {retained}");
    }
    let binding =
        std::fs::read_to_string(root.join("crates/leselang-command/src/result_binding.rs"))
            .unwrap();
    for retained in [
        "validate_captured_bindings(",
        "ProjectedResult::validate",
        "group.validate(groups, &mut validate_group_name)",
        "body.is_result_flow()",
        "validate_captured_projection_accesses(binding, pending, limits.accesses)?",
    ] {
        assert!(binding.contains(retained));
    }
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/projected_group.rs"))
            .unwrap();
    assert!(adapter.contains(
        "validate_projected_group(self, limits, validate_name, ProjectedResult::validate)"
    ));
}

#[test]
fn capture_validation_has_native_order_ownership_and_actual_reference_rejection_proofs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let core = std::fs::read_to_string(
        root.join("crates/leselang-runtime-core/tests/capture_validation.rs"),
    )
    .unwrap();
    for proof in [
        "count_pending_and_scalar_refusals_call_no_native_hook_in_legacy_order",
        "result_and_group_names_interleave_with_once_only_native_calls_and_body_runs_last",
        "invalid_group_names_precede_same_row_duplicates_and_never_run_result_hooks",
        "opaque_move_only_thread_local_payloads_keep_buffers_and_error_ownership_without_trait_bounds",
        "diagnostics_never_invoke_native_formatters_or_expose_original_payloads",
        "callback_unwind_propagates_without_replay_rollback_or_core_field_mutation",
        "unrelated_native_body_and_group_schemas_remain_opaque_and_not_execution_certificates",
    ] {
        assert!(core.contains(proof), "capture validator lost {proof}");
    }
    let vm =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/capture_contract.rs")).unwrap();
    assert!(vm.contains(
        "shared_structure_refusals_keep_lsv1405_and_never_consume_original_pending_authority"
    ));
    assert!(vm.contains("MAX_EFFECT_NESTING_DEPTH + 1"));
    assert!(vm.contains("MAX_BRANCH_NAME_BYTES + 1"));
}

#[test]
fn reference_capture_scope_assembly_uses_shared_nested_entry_but_retains_native_field_and_type_policy()
 {
    assert_workspace_closure("leselang-runtime-core", &[]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/capture_scope.rs"))
            .unwrap();
    let code = shared
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for absent in [
        "leselang_hir",
        "leserpent",
        "rusqlite",
        "serde::",
        ".clone()",
        "unwrap(",
        "expect(",
        "panic!",
        "Fuel::new",
        "HostOperation",
        "ResultField",
    ] {
        assert!(!code.contains(absent), "capture scope gained {absent}");
    }
    for required in [
        "pub fn with_captured_scope",
        "pub enum CaptureScopeError",
        "parent.nested()",
        "let output = (||",
        "drop(scope)",
    ] {
        assert!(code.contains(required));
    }
    assert!(code.find("let output =").unwrap() < code.find("drop(scope)").unwrap());
    assert!(code.find("drop(scope)").unwrap() < code.rfind("output\n}").unwrap());
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-vm/src/result_binding.rs")).unwrap();
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/projection_access.rs"))
            .unwrap();
    for required in [
        "with_captured_scope(",
        "validate_projected_accesses(body, scope, limits).map(|_| ())",
        "CaptureScopeError::Duplicate { .. } => InvalidProjectedResult",
        "CaptureScopeError::Native { error, .. } => error",
        "available_fields(pending).map(AvailableProjection::Fields)",
        "field_bit(field.field)",
        "left & right",
    ] {
        assert!(
            adapter.contains(required),
            "product capture scope lost {required}"
        );
    }
    assert!(reference.contains("leselang_command::validate_result_binding("));
    let binding =
        std::fs::read_to_string(root.join("crates/leselang-command/src/result_binding.rs"))
            .unwrap();
    assert!(
        binding
            .contains("validate_captured_projection_accesses(binding, pending, limits.accesses)?")
    );
    assert!(binding.contains("validate_in_group_scope(&scope, &groups)"));
    assert!(binding.contains("validate_in_scope(&[])"));
}

#[test]
fn shared_capture_scope_has_native_nested_cleanup_and_real_group_successor_compatibility_proofs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/tests/capture_scope.rs"))
            .unwrap();
    for proof in [
        "original_scalar_result_group_pending_order_and_once_only_body_share_the_parent_stack",
        "every_saved_namespace_and_pending_collision_prepares_once_then_restores_original_prefix",
        "native_refusal_precedes_same_row_collision_and_preserves_interleaved_error_position",
        "borrowed_move_only_gui_local_payloads_names_and_outputs_need_no_native_traits",
        "nested_capture_entry_keeps_outer_names_live_and_reuses_closed_inner_names",
        "second_unrelated_native_schema_maps_data_explicitly_without_inferred_projection_or_authority",
        "destructor_unwind_detaches_the_whole_local_suffix_and_preserves_parent_and_once_only_body",
        "early_native_error_is_released_if_previously_prepared_scope_cleanup_unwinds",
    ] {
        assert!(proofs.contains(proof));
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/capture_contract.rs")).unwrap();
    assert!(reference.contains(
        "real_shared_scope_keeps_all_capture_names_distinct_and_group_successor_authority_unchanged"
    ));
    let legacy =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/projection_scope.rs")).unwrap();
    assert!(
        legacy.contains("missing_legacy_fields_remain_rejected_in_cold_bind_loop_and_fold_scopes")
    );
}

#[test]
fn restored_projection_access_walk_is_shared_bounded_and_uses_explicit_native_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/projection_access.rs"))
            .unwrap();
    let production = shared
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for absent in [
        "leserpent",
        "rusqlite",
        "serde::",
        "HostOperation",
        "ResultField",
        ".clone()",
        "unwrap(",
        "expect(",
        "panic!",
        "Fuel::new",
    ] {
        assert!(
            !production.contains(absent),
            "shared projection walk gained {absent}"
        );
    }
    for retained in [
        "pub trait ProjectionAccessPolicy",
        "pub fn observe_projection_accesses",
        "limits.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "StructureBudget::new",
        "check_pending(pending.len(), 1)",
        "scope.nested()",
        "let output =",
        "drop(local)",
        "drop(value)",
        "policy.member(",
        "policy.join(",
    ] {
        assert!(
            shared.contains(retained),
            "shared projection walk lost {retained}"
        );
    }
    assert!(
        shared.find("while let Some((node, depth))").unwrap()
            < shared.find("walk(expression, scope, policy)").unwrap()
    );
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-vm/src/result_binding.rs")).unwrap();
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/projection_access.rs"))
            .unwrap();
    for retained in [
        "observe_projection_accesses(",
        "impl<'a> ProjectionAccessPolicy<'a, ResultField, HostOperation, Effect>",
        "ProjectionAccessError::Native { error, .. } => error",
        "field_bit(*field)",
        "left & right",
        "self.call(operation)",
        "member.result.operation == *operation",
    ] {
        assert!(
            adapter.contains(retained),
            "product projection policy lost {retained}"
        );
    }
    assert!(reference.contains("leselang_command::validate_result_binding("));
    let binding =
        std::fs::read_to_string(root.join("crates/leselang-command/src/result_binding.rs"))
            .unwrap();
    assert!(binding.contains("validate_in_group_scope(&scope, &groups)"));
    assert!(!reference.contains("validate_projected_accesses(value, scope)?"));
    assert!(!adapter.contains("validate_projected_accesses(value, scope)?"));
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/projection_access.rs"))
            .unwrap();
    for proof in [
        "all_control_nodes_follow_original_order_and_cold_scopes_without_evaluation",
        "physical_cold_preflight_rejects_before_all_native_hooks_at_inclusive_boundaries",
        "native_refusals_keep_original_error_identity_phase_and_stop_later_hooks",
        "scope_destructor_unwind_releases_owned_output_and_native_error_before_handoff",
        "field_temporary_destructor_unwind_keeps_native_refusal_locally_owned",
        "conditional_refusal_cleanup_releases_error_if_left_native_destructor_unwinds",
        "independent_borrowed_schema_requires_no_native_traits_or_product_field_masks",
    ] {
        assert!(proofs.contains(proof));
    }
    let vm =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/projection_scope.rs")).unwrap();
    assert!(vm.contains(
        "shared_cold_access_walk_keeps_legacy_fields_closed_in_recovery_calls_and_aliases"
    ));
}

#[test]
fn real_execution_reentry_uses_shared_original_state_without_a_second_meter_or_product_core() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/capture_scope.rs"))
            .unwrap();
    let production = shared
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for absent in [
        "leselang_hir",
        "leserpent",
        "rusqlite",
        "serde::",
        ".clone()",
        "Fuel::new",
        "RefCell",
        "Mutex",
        "unwrap(",
        "expect(",
        "panic!",
    ] {
        assert!(
            !production.contains(absent),
            "shared state entry gained {absent}"
        );
    }
    for retained in [
        "pub struct CaptureScopeHooks",
        "pub fn with_captured_scope_state",
        "State: ?Sized",
        "&mut ()",
        "drop(hooks.pending)",
        "drop(hooks.body)",
        "drop((hooks.scalar, hooks.result, hooks.group))",
        "drop((scalar, result, group))",
    ] {
        assert!(shared.contains(retained));
    }
    let vm =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let production = vm.split("\n#[cfg(test)]\n").next().unwrap();
    for retained in [
        "resume_captured_outcome(binding, ResultView::Group",
        "resume_captured_outcome(binding, ResultView::Raw",
        "crate::resume_preflighted_result_binding_in_scope(",
        "impl crate::ResultCaptureHost for ReferenceCaptureHost",
        "charge_projection(fuel, result)",
        "charge_group_projection(fuel, group)",
        "effect_outcome(outcome.into_legacy())",
        "if self.restore_saved_results",
        "CaptureScopeError::Native { error, .. } => calculation_fault(effect_failure(error))",
    ] {
        assert!(
            production.contains(retained),
            "reference stateful re-entry lost {retained}"
        );
    }
    assert!(!production.contains("for local in &binding.locals"));
    assert!(!production.contains("Fuel::new"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/capture_scope.rs")).unwrap();
    for required in [
        "resume_captured_effects_in_scope(",
        "CapturedEffectHooks {",
        "host.allow_saved_projection()?",
        "host.charge_result(result, fuel)?",
        "host.charge_group(group, fuel)?",
    ] {
        assert!(
            adapter.contains(required),
            "adapter reentry lost {required}"
        );
    }
    let reentry =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_reentry.rs"))
            .unwrap();
    let saved = reentry
        .split("pub fn resume_captured_effects_in_scope")
        .nth(1)
        .unwrap()
        .split("pub type EffectControlResult")
        .next()
        .unwrap();
    for retained in [
        "with_captured_scope_state(",
        "CaptureScopeHooks {",
        "scalar(value, fuel).map_err(native_failure)",
        "result(value, fuel).map_err(native_failure)",
        "group(value, fuel).map_err(native_failure)",
        "pending(fuel).map_err(native_failure)",
        "evaluate_resumable_effects_in_scope(body, scope, environment, fuel, limits)",
    ] {
        assert!(saved.contains(retained), "shared execution lost {retained}");
    }
    assert!(!saved.contains("Fuel::new"));
    assert!(!saved.contains(".clone()"));
    let group =
        std::fs::read_to_string(root.join("crates/leserpent-vm/src/group_binding.rs")).unwrap();
    assert!(
        group.contains("leselang_command::reference_merge::validate_group_binding(self, plan)")
    );
    let reference_merge =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_merge.rs"))
            .unwrap();
    assert!(reference_merge.contains("crate::validate_group_binding("));
    assert!(reference_merge.contains("group_binding_admission_limits()"));
    assert!(reference_merge.contains(".map_err(group_binding_admission_fault)"));
    let group_admission =
        std::fs::read_to_string(root.join("crates/leselang-command/src/group_admission.rs"))
            .unwrap();
    assert!(group_admission.contains(".validate_preflighted_plan(plan, limits.plan)"));
    let group_policy =
        std::fs::read_to_string(root.join("crates/leselang-command/src/group_plan.rs")).unwrap();
    assert!(group_policy.contains("!self.binding.results.is_empty()"));
    assert!(group_policy.contains("!self.binding.groups.is_empty()"));
    let shared_proofs = std::fs::read_to_string(
        root.join("crates/leselang-runtime-core/tests/capture_scope_state.rs"),
    )
    .unwrap();
    for proof in [
        "every_hook_and_body_share_original_move_only_gui_local_context_and_meter",
        "every_exhaustion_boundary_stops_before_later_hooks_without_refund_or_fresh_meter",
        "nested_entry_reborrows_the_same_context_and_original_meter_without_aliasing_or_reset",
        "uncalled_once_hook_destructor_unwind_releases_early_native_error_before_handoff",
        "remaining_mutable_hook_destructor_unwind_releases_output_or_error_after_scope_cleanup",
        "unsized_context_uses_explicit_mutable_reborrows_without_default_or_owned_state",
    ] {
        assert!(shared_proofs.contains(proof));
    }
    for proof in [
        "shared_atomic_and_group_reentry_match_frozen_results_and_every_original_fuel_boundary",
        "shared_duplicate_refusals_preserve_charge_before_insertion_and_original_faults",
        "body_fault_tail_and_successor_capture_keep_original_meter_and_owned_capture",
        "group_reentry_cannot_admit_saved_projection_rows_rejected_by_its_native_contract",
    ] {
        assert!(vm.contains(proof));
    }
    let memory =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/capture_contract.rs")).unwrap();
    assert!(memory.contains(
        "stateful_reentry_keeps_saved_budget_and_once_only_terminal_replay_across_memory_restore"
    ));
}

#[test]
fn real_capture_construction_uses_shared_bounded_snapshot_with_native_charge_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/capture_snapshot.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for absent in [
        "leselang_hir",
        "leserpent",
        "rusqlite",
        "serde::",
        ".clone()",
        "Fuel::new",
        "RefCell",
        "Mutex",
        "unwrap(",
        "expect(",
        "panic!",
    ] {
        assert!(
            !production.contains(absent),
            "shared snapshot gained {absent}"
        );
    }
    for required in [
        "pub fn snapshot_captured_scope_state",
        "State: ?Sized",
        "Source: ?Sized",
        "scope.len() > limits.max_bindings",
        "name.len() > limits.max_name_bytes",
        "scope.bindings().iter().enumerate()",
        "drop(hooks.row)",
    ] {
        assert!(source.contains(required), "shared snapshot lost {required}");
    }
    let vm =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let production = vm.split("\n#[cfg(test)]\n").next().unwrap();
    for required in [
        "crate::PreflightedResultEnvironment::new(",
        "crate::ResultCaptureLimits {",
        "groups: group_capture_limits()",
        "crate::valid_merge_branch_name",
        "CaptureSnapshotError::Native { error, .. } => error",
    ] {
        assert!(
            production.contains(required),
            "reference capture lost {required}"
        );
    }
    assert!(!production.contains("for (name, value) in scope.bindings()"));
    assert!(!production.contains("Fuel::new"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/capture_scope.rs")).unwrap();
    for required in [
        "snapshot_captured_scope_state(",
        "CaptureSnapshotHooks {",
        "host.charge_scalar(value, fuel)?",
        "host.charge_result(&result, fuel)?",
        "host.charge_group(&group, fuel)?",
        "Ok(body.clone())",
    ] {
        assert!(
            adapter.contains(required),
            "adapter snapshot lost {required}"
        );
    }
    for proof in [
        "shared_snapshot_matches_frozen_mixed_scope_bytes_and_every_original_fuel_boundary",
        "shared_snapshot_native_projection_refusal_keeps_original_charge_precedence",
        "snapshot_storage_preflight_rejects_untyped_oversize_before_preparation_without_dispatch",
    ] {
        assert!(vm.contains(proof));
    }
    let tests = std::fs::read_to_string(
        root.join("crates/leselang-runtime-core/tests/capture_snapshot.rs"),
    )
    .unwrap();
    for proof in [
        "ordered_native_preparation_uses_original_gui_local_context_and_moves_category_rows",
        "complete_count_and_utf8_name_preflight_precedes_every_native_hook",
        "uncalled_body_cleanup_unwind_releases_already_owned_row_error",
        "partial_row_cleanup_unwind_releases_later_row_and_body_errors",
        "remaining_row_hook_cleanup_unwind_releases_successful_capture_or_body_error",
        "native_snapshot_and_reentry_round_trip_share_original_meter_without_fresh_grant",
    ] {
        assert!(tests.contains(proof));
    }
    let memory =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/capture_contract.rs")).unwrap();
    assert!(memory.contains(
        "mixed_snapshot_keeps_group_restore_fence_saved_fuel_wire_and_once_only_successors"
    ));
}

#[test]
fn real_shared_control_entries_retain_owned_outputs_through_prefix_and_native_owner_cleanup() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let core =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/owned_scope.rs"))
            .unwrap();
    let production = core
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for absent in [
        "leselang_hir",
        "leserpent",
        "rusqlite",
        "serde::",
        ".clone()",
        "Fuel::new",
        "RefCell",
        "Mutex",
        "unwrap(",
        "expect(",
        "panic!",
    ] {
        assert!(!production.contains(absent), "owned scope gained {absent}");
    }
    assert!(core.contains("State: ?Sized"));
    let output = core.find("let output = body(").unwrap();
    assert!(output < core.find("drop(scope)").unwrap());
    assert!(core.find("drop(scope)").unwrap() < core.find("drop(bindings)").unwrap());
    let single =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_reentry.rs"))
            .unwrap();
    let stacked =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_stack.rs")).unwrap();
    let session =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_session.rs"))
            .unwrap();
    for source in [&single, &stacked, &session] {
        assert!(source.contains("with_owned_scope_state("));
    }
    assert!(single.contains("drop((capture, reply, identity))"));
    assert!(stacked.contains("drop((current, frames, reply, returned, identity))"));
    assert!(stacked.contains("frames.by_ref()"));
    assert!(session.contains("drop(continuation)"));
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "owned_reentry_identity_cleanup_releases_prepared_successor_before_handoff",
        "owned_reentry_prefix_cleanup_releases_prepared_successor_and_preserves_meter",
        "owned_reentry_prefix_cleanup_releases_opaque_projection_error_without_refund",
        "owned_reentry_cold_refusal_cleanup_releases_opaque_error_and_uncalled_capture",
        "owned_reentry_nested_unused_frames_cleanup_releases_opaque_cold_error_once",
        "PRIVATE_ERROR_DROPS",
        "before_drops + 1",
    ] {
        assert!(tests.contains(proof));
    }
}

fn workspace_metadata() -> serde_json::Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata should start");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("cargo metadata should be valid JSON")
}

fn workspace_dependency_graph() -> BTreeMap<String, BTreeSet<String>> {
    let metadata = workspace_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata should contain packages");
    let workspace_names = packages
        .iter()
        .filter(|package| package["source"].is_null())
        .filter_map(|package| package["name"].as_str())
        .collect::<BTreeSet<_>>();

    packages
        .iter()
        .filter_map(|package| {
            let name = package["name"].as_str()?;
            if !workspace_names.contains(name) {
                return None;
            }
            let dependencies = package["dependencies"]
                .as_array()?
                .iter()
                .filter(|dependency| dependency["kind"] != "dev")
                .filter_map(|dependency| dependency["name"].as_str())
                .filter(|dependency| workspace_names.contains(dependency))
                .map(str::to_string)
                .collect();
            Some((name.to_string(), dependencies))
        })
        .collect()
}

fn dependency_closure(graph: &BTreeMap<String, BTreeSet<String>>, root: &str) -> BTreeSet<String> {
    let mut pending = vec![root.to_string()];
    let mut visited = BTreeSet::new();
    while let Some(package) = pending.pop() {
        if !visited.insert(package.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&package) {
            pending.extend(dependencies.iter().cloned());
        }
    }
    visited.remove(root);
    visited
}

fn assert_workspace_closure(root: &str, expected: &[&str]) {
    let graph = workspace_dependency_graph();
    assert!(
        graph.contains_key(root),
        "workspace package '{root}' is missing"
    );
    let actual = dependency_closure(&graph, root);
    let expected = expected
        .iter()
        .map(|dependency| (*dependency).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "'{root}' gained a workspace dependency; review the standalone language boundary"
    );
}

#[test]
fn gewylang_frontend_and_compiler_have_no_product_dependency() {
    assert_workspace_closure("gewylang-contract", &[]);
    assert_workspace_closure("gewylang-ir", &["gewylang-contract"]);
    assert_workspace_closure("gewylang-syntax", &["gewylang-contract"]);
    assert_workspace_closure(
        "gewylang-compiler",
        &["gewylang-contract", "gewylang-syntax"],
    );
}

#[test]
fn leselang_frontend_core_and_reference_facade_have_explicit_dependency_boundaries() {
    assert_workspace_closure("leselang-syntax", &[]);
    assert_workspace_closure("leselang-host-contract", &["silvortex-identity"]);
    assert_workspace_closure(
        "leselang-hir-core",
        &["leselang-runtime-core", "leselang-syntax"],
    );
    assert_workspace_closure(
        "leselang-hir",
        &[
            "leselang-hir-core",
            "leserpent-hir",
            "leselang-host-contract",
            "leselang-runtime-core",
            "leselang-syntax",
            "silvortex-identity",
        ],
    );
    assert_workspace_closure(
        "leserpent-hir",
        &[
            "leselang-hir-core",
            "leselang-host-contract",
            "leselang-runtime-core",
            "leselang-syntax",
            "silvortex-identity",
        ],
    );
}

#[test]
fn shared_control_ir_has_explicit_native_slots_and_reference_aliases_not_a_second_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ir = std::fs::read_to_string(root.join("crates/leselang-hir-core/src/ir.rs")).unwrap();
    for product in [
        "HostOperation",
        "ResultField",
        "crate::Type",
        "leserpent",
        "sqlite",
    ] {
        assert!(!ir.contains(product), "shared control IR leaked {product}");
    }
    assert!(ir.contains("pub enum Computation<Field, Operation, HostEffect, ResultType>"));
    assert!(ir.contains("pub struct ComputedArgument<Expression>"));
    assert!(ir.contains("pub struct ComputedBranch<Expression, ResultType>"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(reference.contains(
        "pub type Computation = crate::ir::Computation<ResultField, HostOperation, Effect, Type>"
    ));
    assert!(
        reference
            .contains("pub type ComputedBranch = crate::ir::ComputedBranch<Computation, Type>")
    );
    assert!(
        reference.contains("pub type ComputedArgument = crate::ir::ComputedArgument<Computation>")
    );
    assert!(!reference.contains("pub enum Computation"));
    assert!(!reference.contains("fn children("));
    assert!(!reference.contains("fn is_pure("));
}

#[test]
fn shared_argument_typing_uses_native_domains_and_the_reference_ir_bridge() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let checker =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/argument_typing.rs"))
            .unwrap();
    for product in [
        "leselang_hir",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !checker.contains(product),
            "shared argument typing leaked {product}"
        );
    }
    assert!(checker.contains("pub trait ScalarArgumentDomain"));
    assert!(checker.contains("pub fn check_argument_types("));
    let lower =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(lower.contains("bind_source_arguments("));
    assert!(lower.contains("Type::Scalar(ty) => Some(ty)"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/source_call.rs")).unwrap();
    assert!(source.contains("check_argument_type(&parameter.domain, facts)"));
    assert!(source.contains("is_pure: true"));
    let domains =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/host_call.rs")).unwrap();
    assert!(domains.contains("impl ScalarArgumentDomain for ArgumentDomain"));
    assert!(domains.contains("check_argument_type(&self, ScalarArgumentType::literal(value))"));
}

#[test]
fn native_source_call_bridge_uses_original_ast_catalog_and_shared_ir_without_product_types() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/source_call.rs")).unwrap();
    for product in [
        "HostOperation",
        "ResultField",
        "crate::Type",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "source call bridge leaked {product}"
        );
    }
    assert!(source.contains("use leselang_syntax::{Expression, NamedArgument, Span};"));
    assert!(source.contains("use crate::ir::{Computation, ComputedArgument};"));
    assert!(source.contains("pub fn lower_source_call"));
    let public = source.split_once("pub fn lower_source_call").unwrap().1;
    assert!(
        public.find("preflight(expression, limits)?;").unwrap()
            < public
                .find(".authorize(callee.as_str(), host.version, host.granted)")
                .unwrap()
    );
    assert!(
        public
            .find(".bind_arguments(&names[..arguments.len()])")
            .unwrap()
            < public.find("lower(argument)").unwrap()
    );
    assert!(public.contains("preflight_with_budget(&value, 1, &mut budget)"));
    assert!(public.find(".check_pending(0, 1)").unwrap() < public.find("lower(argument)").unwrap());
    assert!(!source.contains(".scalar_argument_type("));
    assert!(!source.contains("pub enum Expression"));
    assert!(!source.contains(".clone()"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("return lowered\n            .finish_call("));
    assert!(!reference.contains("let bindings = operation.bind_names(&names"));
}

#[test]
fn prepared_native_calls_delegate_owned_construction_and_require_fresh_whole_call_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/source_call.rs")).unwrap();
    let finish = source
        .split_once("pub fn finish_call")
        .unwrap()
        .1
        .split_once("impl<Key, Domain")
        .unwrap()
        .0;
    assert!(finish.contains("map: impl FnOnce("));
    assert!(finish.contains("admit: impl FnOnce("));
    assert!(
        finish
            .find("preflight_with_budget(&argument.value, 1, &mut budget)")
            .unwrap()
            < finish.find("map(schema)").unwrap()
    );
    assert!(
        finish.find("let call = Computation::Call").unwrap()
            < finish.find("admit(&call, &output, schema)").unwrap()
    );
    assert!(finish.contains("arguments: self.into_arguments()"));
    assert!(finish.contains("Ok((call, output))"));
    for forbidden in [".clone()", "serde", "Fuel", "HostOperation", "crate::Type"] {
        assert!(
            !finish.contains(forbidden),
            "native call completion leaked {forbidden}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("SourceCallFinishLimits"));
    assert!(reference.contains("std::ptr::eq(schema, operation.schema())"));
    assert!(reference.contains("crate::pure_reference::infer_source_call_in_scope("));
    assert!(!reference.contains("arguments: lowered.into_arguments()"));
    let native =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    let native = native
        .split_once("pub(crate) fn infer_source_call_in_scope")
        .unwrap()
        .1
        .split_once("fn limits()")
        .unwrap()
        .0;
    assert!(native.contains("scope\n        .bindings()"));
    assert!(native.contains("local.members().map(ReferenceMembers::External)"));
    assert!(native.contains("max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS"));
    assert!(native.contains("check_call_arguments("));
    for recursive in [
        "validate_in_type_scope",
        "canonical_source",
        "lower_expression",
        ".clone()",
    ] {
        assert!(
            !native.contains(recursive),
            "source admission reentered/copied {recursive}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/source_call_finish.rs"))
            .unwrap();
    for claim in [
        "exact_original_operands_schema_and_move_only_outputs_reach_once_only_admission",
        "complete_cold_forest_uses_one_current_root_node_and_depth_budget",
        "fresh_lexical_admission_rejects_forged_prepared_scalar_facts",
        "original_schema_borrow_does_not_freeze_live_native_domains_version_or_grants",
        "native_admission_unwind_releases_whole_owned_output_without_refunding_external_work",
        "unrelated_numeric_and_panel_hosts_complete_original_calls_and_prepare_actual_values",
        "reference_computed_call_keeps_full_width_group_scope_and_canonical_wire_policy",
    ] {
        assert!(proof.contains(claim), "missing native call proof: {claim}");
    }
}

#[test]
fn pure_type_inference_uses_shared_ir_and_native_queries_not_reference_product_types() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/pure_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "pure inference leaked {product}");
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub trait PureTypeEnvironment<Field, Operation>"));
    assert!(source.contains("pub fn infer_pure_type"));
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let canonical_gate = reference.find("if roundtrip != *expression").unwrap();
    let native_check = reference
        .find("crate::pure_reference::infer(expression, scope, groups)")
        .unwrap();
    assert!(canonical_gate < native_check);
    assert!(reference.contains("crate::pure_typing::valid_local_name(name)"));
}

#[test]
fn atomic_call_inference_uses_native_catalogs_and_shared_types_not_product_commands() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/call_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "atomic call inference leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::{Computation, ComputedArgument};"));
    assert!(source.contains("pub fn infer_call_type"));
    assert!(source.contains("pub fn check_call_arguments"));
    assert!(source.contains("OperationCatalog"));
    assert!(source.contains("infer_in_scope"));
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(
        reference.find("if roundtrip != *expression").unwrap()
            < reference.find("crate::pure_reference::infer_call").unwrap()
    );
}

#[test]
fn prepared_call_typing_keeps_one_ir_and_preflights_cold_schemas_before_type_metadata() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/prepared_typing.rs"))
            .unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "prepared call inference leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub fn infer_prepared_call_type"));
    assert!(source.contains("pub trait PreparedCallSchemas"));
    assert!(source.contains("std::ptr::eq(expected, schema)"));
    assert!(source.find(".select(operation)").unwrap() < source.find("bindings.to_vec()").unwrap());
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let canonical = reference.find("if roundtrip != *expression").unwrap();
    let prepared = reference
        .find("crate::pure_reference::infer_prepared")
        .unwrap();
    assert!(canonical < prepared);
    assert!(reference.contains("crate::pure_reference::call_leaves_only(expression)"));
}

#[test]
fn call_result_dataflow_uses_explicit_native_mapping_after_all_cold_schemas() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/flow_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "call flow typing leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub trait CallFlowEnvironment"));
    assert!(source.contains("pub fn infer_call_flow_type"));
    assert!(source.contains("check_in_scope"));
    assert!(source.contains("infer_in_scope"));
    assert!(source.find(".select(operation)").unwrap() < source.find("bindings.to_vec()").unwrap());
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(
        reference.find("if roundtrip != *expression").unwrap()
            < reference.find("crate::pure_reference::infer_flow").unwrap()
    );
    assert!(reference.contains("crate::pure_reference::call_flow_nodes_only(expression)"));
    assert!(reference.contains("if !prepared_call"));
}

#[test]
fn group_dataflow_keeps_one_engine_and_checks_cold_rows_before_native_exports() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/flow_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "group dataflow leaked {product}");
    }
    assert!(source.contains("pub trait GroupFlowEnvironment"));
    assert!(source.contains("pub fn infer_group_flow_type"));
    assert!(source.contains("pub struct GroupMemberType"));
    assert!(source.contains("preflight_call_leaves_with_budget"));
    assert!(source.contains("std::ptr::eq(row, schema)"));
    assert!(
        source
            .find("for (group_index, group) in physical.groups")
            .unwrap()
            < source.find("bindings.to_vec()").unwrap()
    );
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(
        reference.find("if roundtrip != *expression").unwrap()
            < reference
                .find("crate::pure_reference::infer_group_flow")
                .unwrap()
    );
    assert!(reference.contains("crate::pure_reference::group_flow_nodes_only(expression)"));
}

#[test]
fn opaque_host_dataflow_uses_the_same_engine_with_explicit_cold_admission_and_borrowed_types() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let flow =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/flow_typing.rs")).unwrap();
    for marker in [
        "pub trait HostFlowEnvironment<'expression",
        "pub struct HostFlowTypeLimits",
        "pub fn infer_host_flow_type",
        "fn host_result_type(&self, effect: &'expression HostEffect)",
        "fn infer_with_policies",
        "host_limit: Some(limits.max_hosts)",
        "HostAdmission { host_index }",
        "HostResultType { host_index }",
        "host_limit: None",
        "hosts: &NoHosts",
    ] {
        assert!(
            flow.contains(marker),
            "missing opaque host boundary {marker}"
        );
    }
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "serde::",
        "evaluate_",
        "canonical_source(",
    ] {
        assert!(!flow.contains(product), "opaque flow leaked {product}");
    }
    let assembly = flow.split_once("fn infer_with_policies").unwrap().1;
    let physical = assembly.find("let physical = preflight(").unwrap();
    let calls = assembly.find(".select(operation)").unwrap();
    let admission = assembly.find("policies.hosts.admit(effect)").unwrap();
    let clone = assembly.find("bindings.to_vec()").unwrap();
    assert!(physical < calls && calls < admission && admission < clone);
    let product =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(
        product.find("if roundtrip != *expression").unwrap()
            < product
                .find("crate::pure_reference::infer_host_flow")
                .unwrap()
    );
    assert!(product.contains("crate::pure_reference::host_flow_nodes_only("));
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    assert!(adapter.contains("infer_host_flow_type("));
    let native = adapter
        .split_once("fn admit_host")
        .unwrap()
        .1
        .split_once("impl<'a> PureTypeEnvironment")
        .unwrap()
        .0;
    assert!(native.contains("HostOperation::for_effect(effect)"));
    assert!(!native.contains("validate_in_type_scope") && !native.contains("canonical_source("));
    let proof = std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_flow_typing.rs"))
        .unwrap();
    for boundary in [
        "cold_native_admission_precedes_prefix_clone_and_guard_or_field_queries",
        "all_cold_call_policy_checks_precede_opaque_admission_and_semantic_queries",
        "hidden_native_graph_foreign_owner_schema_version_and_grants_need_explicit_admission",
        "guards_call_arguments_and_projection_loop_fold_recovery_operands_stay_pure_even_cold",
        "native_hook_unwind_drops_temporary_scope_without_prefix_mutation_or_retry",
        "result_metadata_can_borrow_the_original_nonclone_opaque_declaration",
        "unrelated_gui_host_uses_nonclone_slots_and_original_owned_declaration_identity",
        "exact_large_prefix_ceiling_and_active_growth_are_not_residual_small_product_limits",
    ] {
        assert!(proof.contains(boundary));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_flow_reference.rs"))
            .unwrap();
    for boundary in [
        "mixed_native_host_and_computed_call_chains_keep_native_types_wire_and_authority",
        "native_graph_host_wrappers_and_new_groups_keep_their_existing_source_policy",
        "native_raw_receipt_and_effectful_operand_restrictions_do_not_become_typing_permissions",
        "forged_native_payload_and_literal_call_nodes_are_still_rejected_by_canonical_admission",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn leselang_runtime_core_has_no_workspace_or_build_dependency_even_in_its_tests() {
    assert_workspace_closure("leselang-runtime-core", &[]);
    let metadata = workspace_metadata();
    let core = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "leselang-runtime-core")
        .expect("standalone runtime foundation must exist");
    let actual: BTreeSet<_> = core["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|dependency| {
            assert!(
                dependency["path"].is_null(),
                "core must not need workspace paths"
            );
            (
                dependency["name"].as_str().unwrap().to_string(),
                dependency["kind"].as_str().unwrap_or("normal").to_string(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
            ("serde".to_string(), "normal".to_string()),
            ("serde_json".to_string(), "dev".to_string())
        ]
        .into()
    );
    assert!(!core["targets"].as_array().unwrap().iter().any(|target| {
        target["kind"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kind| kind == "custom-build")
    }));

    let workflow = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml"),
    )
    .unwrap();
    let standalone_job = workflow
        .split_once("  leselang-core:\n")
        .unwrap()
        .1
        .split_once("  rust:\n")
        .unwrap()
        .0;
    let package = standalone_job
        .find("cargo +1.98.0 package -p leselang-runtime-core --locked")
        .expect("standalone package verification must remain locked");
    let fetch = standalone_job
        .find("cargo +1.98.0 fetch --manifest-path target/package/leselang-runtime-core-*/Cargo.toml --locked")
        .expect("normalized package dev-dependencies must be fetched before offline tests");
    let test = standalone_job
        .find("cargo +1.98.0 test --manifest-path target/package/leselang-runtime-core-*/Cargo.toml --locked --offline")
        .expect("standalone tests must remain locked and offline");
    assert!(
        package < fetch && fetch < test,
        "fetch must use the verified package's lockfile before offline tests"
    );
    assert!(workflow.contains("needs: [rust, product-surfaces, leselang-core]"));
}

#[test]
fn mixed_host_groups_share_atomic_walk_typing_and_exact_native_declaration_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let flow =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/flow_typing.rs")).unwrap();
    for marker in [
        "pub trait HostGroupFlowEnvironment",
        "pub struct HostGroupFlowTypeLimits",
        "pub fn infer_host_group_flow_type",
        "preflight_atomic_leaves_with_budget",
        "group_hosts: &NoGroupHosts",
        "std::ptr::eq(row, schema)",
    ] {
        assert!(
            flow.contains(marker),
            "missing mixed group boundary {marker}"
        );
    }
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "canonical_source(",
        "validate_in_type_scope",
        "evaluate_",
        "serde::",
    ] {
        assert!(
            !flow.contains(coupling),
            "mixed group typing leaked {coupling}"
        );
    }
    let assembly = flow.split_once("fn infer_with_policies").unwrap().1;
    assert!(
        assembly.find("policies.hosts.admit(effect)").unwrap()
            < assembly
                .find("policies.group_hosts.operation(effect)")
                .unwrap()
    );
    assert!(
        assembly
            .find(".admit(effect, operation, &schema.result)")
            .unwrap()
            < assembly.find("bindings.to_vec()").unwrap()
    );
    assert!(
        assembly.find("std::ptr::eq(row, schema)").unwrap()
            < assembly.find("bindings.to_vec()").unwrap()
    );
    let prepared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/prepared_typing.rs"))
            .unwrap();
    assert!(prepared.contains(
        "preflight_atomic_leaves_with_budget(expression, depth, budget, max_arguments, false)"
    ));
    assert!(prepared.contains("Computation::Host { .. } if allow_hosts"));
    assert!(prepared.contains("call_index: call_count"));
    let product =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(
        product.find("if roundtrip != *expression").unwrap()
            < product
                .find("crate::pure_reference::infer_host_group_flow")
                .unwrap()
    );
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    assert!(adapter.contains("infer_host_group_flow_type("));
    assert!(adapter.contains("std::ptr::eq(declaration, &operation.schema().result)"));
    assert!(adapter.contains("std::ptr::eq(operation, &operation.schema().result.operation)"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_group_typing.rs"))
            .unwrap();
    for boundary in [
        "mixed_sequence_parallel_exports_borrow_original_names_and_host_call_operations",
        "pure_preparation_and_conditional_host_call_paths_require_one_original_schema_row",
        "native_pair_admission_rejects_equal_lookalike_keys_foreign_results_and_missing_mapping",
        "group_member_pure_initializers_and_cold_call_positions_do_not_count_host_leaves_as_calls",
        "native_mapping_pair_declaration_and_construction_unwind_never_publish_or_retry_members",
        "unrelated_gui_profile_borrows_nonclone_operation_effect_and_result_declarations",
    ] {
        assert!(proof.contains(boundary));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_group_reference.rs"))
            .unwrap();
    assert!(
        proof.contains(
            "mixed_flat_sequence_parallel_groups_keep_product_types_wire_and_capabilities"
        )
    );
    assert!(proof.contains(
        "forged_group_result_operation_and_native_payload_are_rejected_before_shared_type_exports"
    ));
}

#[test]
fn flat_native_graph_types_reuse_shared_protocols_and_original_closed_member_observations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    for marker in [
        "pub(crate) fn flat_native_group(",
        "pub(crate) fn supports_host_flow(",
        "operation.schema().result.ty != branch.result_type",
        "branch.name.as_str()",
        "members: Some(ReferenceMembers::Computed { kind, members })",
        "native_group_type_observations_borrow_names_and_exact_canonical_operation_rows",
        "native_and_computed_group_joins_ignore_payloads_but_never_union_exports",
    ] {
        assert!(
            adapter.contains(marker),
            "missing flat native graph boundary {marker}"
        );
    }
    assert!(
        adapter
            .split_whitespace()
            .collect::<String>()
            .contains("&member.operation.schema().result.operation")
    );
    let product =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(product.contains("crate::pure_reference::native_group_exports(effect)"));
    assert_eq!(
        product
            .matches("crate::pure_reference::supports_host_flow")
            .count(),
        2
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_flow.rs"))
            .unwrap();
    for marker in [
        "independent_gui_graph_exports_borrow_original_names_operations_and_nonclone_declarations",
        "native_and_computed_gui_groups_join_only_the_same_original_closed_signature",
        "every_cold_native_graph_admission_precedes_prefix_clones_and_member_queries",
        "host_language_limits_do_not_replace_inclusive_private_graph_limits_or_live_policy",
        "changed_native_graph_metadata_needs_fresh_admission_without_a_cached_certificate",
        "nested_opaque_graphs_are_not_accepted_as_atomic_computed_members",
        "native_graph_hook_unwind_does_not_retry_publish_or_mutate_the_borrowed_prefix",
        "call_policy_and_impure_cold_guards_precede_all_opaque_graph_queries",
    ] {
        assert!(proof.contains(marker));
    }
    for coupling in [
        "leserpent",
        "HostOperation",
        "Effect::",
        "canonical_source",
        "serde::",
        "tokio",
    ] {
        assert!(
            !proof.contains(coupling),
            "independent graph proof leaked {coupling}"
        );
    }
    let proof = std::fs::read_to_string(
        root.join("crates/leselang-hir/tests/native_group_flow_reference.rs"),
    )
    .unwrap();
    for marker in [
        "flat_native_group_captures_and_result_driven_successors_keep_wire_and_authority",
        "native_and_computed_group_choices_share_one_closed_ordered_signature",
        "native_group_aliases_pure_selection_and_helpers_keep_closed_exports",
        "mode_order_names_and_operation_mismatches_do_not_union_native_and_computed_exports",
        "cold_forged_native_graph_metadata_and_payloads_fail_canonical_admission",
        "an_opaque_group_cannot_masquerade_as_an_atomic_computed_member",
    ] {
        assert!(proof.contains(marker));
    }
}

#[test]
fn actual_result_validation_is_product_free_and_shared_by_reference_raw_value_binding() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/host_result.rs"))
            .unwrap();
    for product in [
        "leselang_hir",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "host-result validation leaked {product}"
        );
    }
    assert!(source.contains("pub trait HostResultDomain<Reply: ?Sized>"));
    assert!(source.contains("pub fn validate_host_result"));
    assert!(
        source.find("if !domain.matches_type(reply)").unwrap()
            < source.find(".validate_value(reply)").unwrap()
    );
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_result.rs"))
            .unwrap();
    assert!(reference.contains("impl ResultValueHost for BoundResultHost"));
    assert!(reference.contains("let declaration = ResultValueDomain {"));
    assert!(reference.contains("PendingReply::new(image.token.as_str(), image, &declaration)"));
    assert!(reference.contains(".try_accept(&image.token.as_str(), value, &ResultObservation)"));
    assert!(reference.contains(
        "ReplyAcceptanceError::Result(HostResultError::InvalidValue(error)) => match error"
    ));
    assert!(reference.contains("ResultValueError::Encoding(error) => error"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/result_domain.rs")).unwrap();
    assert!(adapter.contains("HostResultDomain<Value> for ResultValueDomain"));
    assert!(adapter.contains("reply.result_type() == self.result_type"));
    let vm =
        std::fs::read_to_string(root.join("crates/leserpent-vm/src/leserpent_profile.rs")).unwrap();
    assert!(vm.contains("use host_result::validate_bound_value;"));
    assert!(vm.contains("leselang_command::reference_result::step_from_effect_result("));
    assert!(reference.contains("match accept_bound_value(image, &value)"));
    let bridge =
        std::fs::read_to_string(root.join("crates/leserpent-vm/src/host_result.rs")).unwrap();
    assert!(
        bridge.contains("pub(super) use leselang_command::reference_result::validate_bound_value;")
    );
    assert!(!vm.contains("fn validate_bound_value("));
}

#[test]
fn pending_reply_ownership_is_product_free_and_closes_before_native_callbacks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/reply.rs")).unwrap();
    for product in [
        "leselang_hir",
        "ContinuationImage",
        "leserpent",
        "sqlite",
        "serde",
    ] {
        // Documentation names serde only to reject the requirement; no import/derive is allowed.
        if product == "serde" {
            assert!(!source.contains("use serde"));
            assert!(!source.contains("Serialize, Deserialize"));
        } else {
            assert!(
                !source.contains(product),
                "reply ownership leaked {product}"
            );
        }
    }
    assert!(source.contains("pub trait ReplyAuthority<Identity>"));
    assert!(source.contains("Reply: Borrow<View>"));
    let attempt = source.split_once("pub fn try_accept").unwrap().1;
    let close = attempt
        .find("State::Terminal(ReplyEnd::HostUncertain)")
        .unwrap();
    let identity = attempt
        .find("&waiting.identity != actual_identity")
        .unwrap();
    let authority = attempt
        .find("authority.authorize(&waiting.identity)")
        .unwrap();
    let value = attempt.find("validate_host_result(").unwrap();
    assert!(close < identity && identity < authority && authority < value);
    assert!(attempt.contains("self.state = State::Waiting(waiting)"));
    assert!(
        attempt
            .find("self.state = State::Terminal(ReplyEnd::Accepted)")
            .unwrap()
            < attempt.find("Ok(AcceptedReply {").unwrap()
    );
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_evaluation.rs"))
            .unwrap();
    assert!(proofs.contains("PendingReply::new((7, 1), capture"));
    assert!(proofs.contains("PendingReply::new(1u64, capture"));
    assert!(proofs.contains("ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)"));
}

#[test]
fn shared_pure_execution_is_product_free_and_used_by_the_reference_vm() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/pure_evaluation.rs"))
            .unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "shared pure execution leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub trait PureEvaluationEnvironment<Field, Operation>"));
    assert!(source.contains("pub fn evaluate_pure_in_scope"));
    assert!(source.contains("preflight_with_budget(expression, 0, &mut budget)"));
    assert!(source.contains("Err(failure) if failure.is_recoverable()"));
    assert!(!source.contains("pub enum Computation"));
    let vm =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    assert!(vm.contains("type LocalValue<'a> = PureValue<ResultView<'a>>"));
    assert!(vm.contains("evaluate_effects_in_scope("));
    let control =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_evaluation.rs"))
            .unwrap();
    assert!(control.contains("evaluate_preflighted_in_scope("));
    assert!(vm.contains("PureEvaluationFault::Native(fault) => fault"));
    for duplicated in [
        "LoopBudget::new",
        "FoldCursor::new",
        "StringListBuilder::with_capacity",
        "apply_binary(",
        "apply_unary(",
    ] {
        assert!(
            !vm.contains(duplicated),
            "reference VM duplicated {duplicated}"
        );
    }
}

#[test]
fn atomic_call_execution_reuses_original_schema_control_and_argument_engines_without_a_new_meter() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/prepared_call_evaluation.rs"),
    )
    .unwrap();
    let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");
    for boundary in [
        "preflight_call_leaves_with_budget",
        "type Capture = Infallible",
        "evaluate_arguments(",
        "std::ptr::eq(*original, expression)",
        "Self::Error::Projection",
        "Self::Error::Arguments",
        "ResumableEffectOutcome::Request(request) => Ok(request)",
    ] {
        assert!(
            normalized.contains(boundary),
            "atomic execution lost {boundary}"
        );
    }
    let entry = normalized
        .split("pub fn prepare_atomic_call_in_scope")
        .nth(1)
        .unwrap()
        .split("fn select_atomic<")
        .next()
        .unwrap();
    let mut previous = 0;
    for gate in [
        "preflight_value_scope(scope, limits.pure)",
        "preflight_call_leaves_with_budget(expression, 0, &mut budget, limits.max_arguments)",
        "let adapter = select_atomic(leaves, host, limits)?",
        "execute_atomic(expression, scope, &adapter, fuel)",
    ] {
        let position = entry
            .find(gate)
            .unwrap_or_else(|| panic!("missing atomic gate {gate}"));
        assert!(position > previous, "atomic gate order changed for {gate}");
        previous = position;
    }
    let selection = normalized
        .split("fn select_atomic<")
        .nth(1)
        .unwrap()
        .split("fn execute_atomic<")
        .next()
        .unwrap();
    let mut previous = 0;
    for gate in [
        ".authorize(operation, host.version, host.granted)",
        "preflight_signature(&names, schema, limits.max_arguments)",
        "std::ptr::eq(row, schema)",
        "Ok(Atomic {",
    ] {
        let position = selection.find(gate).unwrap();
        assert!(position > previous);
        previous = position;
    }
    assert!(
        source
            .split_whitespace()
            .collect::<String>()
            .contains("evaluate(expression,scope,adapter,fuel,adapter.limits.pure.max_bindings,)")
    );
    for excluded in [
        "Fuel::new",
        "fuel.charge",
        "bindings.to_vec()",
        "scope.bindings().to_vec()",
        "Operation: Clone",
        "Field: Clone",
        "Capability: Clone",
        "ResultTag: Clone",
        "unsafe",
        "sqlite",
        "HostOperation",
        "serde::",
        "pub enum Computation",
    ] {
        assert!(
            !source.contains(excluded),
            "atomic preparation introduced {excluded}"
        );
    }
    let cases =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_call_evaluation.rs"))
            .unwrap();
    for proof in [
        "uniform_original_schema_prepares_one_selected_call_in_live_scope_without_capture_or_prefix_copy",
        "direct_call_cost_and_native_argument_failure_match_the_released_entry",
        "cold_physical_flow_limits_and_prefix_reject_before_native_comparisons_or_execution",
        "cold_unknown_wrong_version_missing_grant_named_shape_and_equal_result_different_operation_are_denied",
        "native_projection_errors_remain_move_only_private_and_cannot_be_scalar_recovered",
        "native_unwind_drops_temporary_bindings_without_capture_retry_or_fuel_reset",
        "prepared_bind_choose_members_reach_both_original_reply_protocols_without_a_second_meter",
    ] {
        assert!(cases.contains(proof));
    }
}

#[test]
fn shared_call_preparation_connects_native_catalogs_and_values_to_the_reference_vm() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/call_evaluation.rs"))
            .unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "call preparation leaked {product}"
        );
    }
    for shared in [
        "use crate::ir::{Computation, ComputedArgument};",
        "pub fn prepare_call_in_scope",
        "pub fn evaluate_call_arguments_in_scope",
        "pub struct PreparedCall",
        "preflight_value_scope",
        "preflight_arguments_with_budget",
        "evaluate_preflighted_in_scope",
        "check_argument_type",
    ] {
        assert!(
            source.contains(shared),
            "missing shared call boundary {shared}"
        );
    }
    assert!(
        source
            .find("let names = preflight(arguments, scope, limits)?;")
            .unwrap()
            < source.find("evaluate_arguments(arguments, &names").unwrap()
    );
    assert!(!source.contains("bindings.to_vec()"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/host_call.rs")).unwrap();
    assert!(reference.contains("pub fn evaluate_computed_arguments"));
    assert!(reference.contains("crate::call_evaluation::evaluate_call_arguments_in_scope("));
    let vm =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    assert!(vm.contains("crate::PreflightedResultEnvironment::new("));
    assert!(!vm.contains("for argument in arguments"));
    assert!(
        vm.contains("CallEvaluationError::Evaluation { failure, .. } => pure_failure(failure)")
    );
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-command/src/effect_preparation.rs"))
            .unwrap();
    assert!(adapter.contains(".evaluate_computed_arguments("));
    assert!(adapter.contains("operation.resolve(&values)"));
}

#[test]
fn atomic_group_preparation_shares_whole_cold_admission_and_original_execution_without_a_new_walker()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/prepared_call_evaluation.rs"),
    )
    .unwrap();
    let text = source.split_whitespace().collect::<Vec<_>>().join(" ");
    let group = text
        .split("pub fn prepare_atomic_group_in_scope")
        .nth(1)
        .unwrap();
    let mut previous = 0;
    for gate in [
        "preflight_value_scope(scope, limits.call.pure)",
        "preflight_native_group_members(",
        "budget .visit(0, 0, 0)",
        "preflight_call_leaves_with_budget( &branch.value, 1, &mut budget",
        "select_atomic(leaves, host, limits.call)",
        "prepare_native_group_members(",
        "admit(index, branch, adapter.schema)",
        "execute_atomic(&branch.value, scope, adapter, fuel)",
    ] {
        let position = group
            .find(gate)
            .unwrap_or_else(|| panic!("missing whole-group gate {gate}"));
        assert!(position > previous, "group gate order changed for {gate}");
        previous = position;
    }
    for forbidden in [
        "Fuel::new",
        "fuel.charge",
        "bindings.to_vec",
        "Operation: Clone",
        "IrResult: Clone",
        "ResultTag: Clone",
        "enum State",
        "leselang_vm",
        "rusqlite",
        "serde::",
    ] {
        assert!(
            !source.contains(forbidden),
            "whole preparation gained {forbidden}"
        );
    }
    assert_eq!(
        text.matches("let schema = host .catalog .authorize(")
            .count(),
        1
    );
    assert_eq!(text.matches("match evaluate(").count(), 1);
    let cases =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_call_evaluation.rs"))
            .unwrap();
    for proof in [
        "complete_atomic_group_bridge_preserves_original_rows_scope_once_order_and_one_meter",
        "aggregate_shifted_atomic_group_physical_and_scope_denials_precede_catalog_or_native_hooks",
        "every_cold_atomic_group_catalog_signature_and_uniform_row_precedes_any_value_preparation",
        "inclusive_atomic_group_forest_and_sixty_four_member_limits_do_not_reset_per_member",
        "complete_atomic_group_native_admission_failure_or_unwind_precedes_fuel_and_projections",
        "atomic_group_execution_exhaustion_returns_no_partial_batch_or_scope_leak_or_refund",
        "late_atomic_group_domain_failure_or_unwind_cleans_prepared_prefix_and_temporary_views",
    ] {
        assert!(cases.contains(proof));
    }
    let vertical =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    assert!(
        vertical
            .matches("ParamRuntime::with_prepared_groups(&domain)")
            .count()
            >= 3
    );
    assert!(vertical.contains("let prepared = prepare_atomic_group_in_scope("));
    assert!(vertical.contains("assert_eq!(session.fuel_remaining(), 244)"));
}

#[test]
fn shared_effect_control_drives_reference_vm_with_explicit_non_dispatching_capture_hooks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_evaluation.rs"))
            .unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "effect control leaked {product}");
    }
    for boundary in [
        "pub trait EffectEvaluationEnvironment",
        "pub enum EffectEvaluationOutcome",
        "pub fn evaluate_effects_in_scope",
        "fn preflight_effect(",
        "fn prepare_effect(",
        "fn capture(",
        "let mut local = scope.nested();",
        "evaluate_preflighted_in_scope(",
        "EffectEvaluationFault::NestedSuspension",
    ] {
        assert!(
            source.contains(boundary),
            "missing effect control boundary {boundary}"
        );
    }
    assert!(
        source
            .find("let effects = preflight(expression, scope, limits)?;")
            .unwrap()
            < source.find(".preflight_effect(effect)").unwrap()
    );
    assert!(!source.contains("pub enum Computation"));
    assert!(!source.contains("bindings.to_vec()"));
    let vm =
        std::fs::read_to_string(root.join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    assert!(vm.contains("evaluate_effects_in_scope("));
    assert!(vm.contains("crate::PreflightedResultEnvironment::new("));
    let environment =
        std::fs::read_to_string(root.join("crates/leselang-command/src/execution_environment.rs"))
            .unwrap();
    assert!(environment.contains("type Capture = Box<ResultBinding>;"));
    assert!(environment.contains("snapshot_preflighted_result_scope("));
    assert!(vm.contains("charge_group_projection(fuel, group)"));
    assert!(!vm.contains("Computation::Bind { name, value, body } =>"));
    assert!(!vm.contains("Err(failure) if failure.is_recoverable()"));
}

#[test]
fn shared_scalar_source_construction_is_product_free_and_delegated_by_reference_compiler() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/scalar_source.rs"))
            .unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "scalar source leaked {product}");
    }
    for boundary in [
        "pub fn lower_scalar_source",
        "pub(crate) fn lower_form",
        "pub(crate) fn lower_choose_form",
        "StringListBuilder::with_capacity",
        ".try_push(text)",
        "check_string_label",
        "preflight(expression, limits)",
        "preflight_names(expression)",
        "preflight_with_budget(&value",
        "BinaryOperator::parse",
        "UnaryOperator::parse",
        "ScalarSourceError::InconsistentLiteral",
        "type OperandResult",
        "if depth > self.max_depth",
    ] {
        assert!(
            source.contains(boundary),
            "missing scalar source boundary {boundary}"
        );
    }
    let public = source.split_once("pub fn lower_scalar_source").unwrap().1;
    assert!(
        public.find("preflight(expression, limits)").unwrap()
            < public.find("preflight_names(expression)").unwrap()
    );
    assert!(
        public.find("preflight_names(expression)").unwrap()
            < public.find("let output = Builder").unwrap()
    );
    assert!(!source.contains("apply_binary("));
    assert!(!source.contains("apply_unary("));
    assert!(!source.contains("Some(text.clone())"));
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("crate::scalar_form::bind_scalar_form(expression)"));
    assert!(reference.contains(".lower(|child|"));
    assert!(reference.contains("crate::choice_source::lower_choice_source("));
    assert!(reference.contains("crate::binding_source::lower_binding_source("));
    assert!(reference.contains("source.construct(value, body)"));
    assert!(!reference.contains("if callee == \"recover\""));
    assert!(!reference.contains("if callee == \"strings\""));
    assert!(!reference.contains("fn unary_type("));
    assert!(!reference.contains("if let Some(operator) = BinaryOperator::parse(callee)"));
    assert!(!reference.contains("Some(text.clone())"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/source_call.rs")).unwrap();
    assert!(proof.contains("leselang_hir::scalar_source::lower_scalar_source"));
    assert!(!proof.contains("fn node(expression: &Expression)"));
}

#[test]
fn shared_scalar_source_bindings_keep_explicit_policy_and_read_only_native_scope() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/scalar_source.rs"))
            .unwrap();
    for boundary in [
        "pub struct ScalarSourceLimits",
        "pub max_bindings: usize",
        "pub struct ScalarSourceScope",
        "pub fn get(&self, name: &str) -> Option<ScalarType>",
        "max_bindings > MAX_TYPE_INFERENCE_BINDINGS",
        "pub fn lower_scalar_source_with_scope",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
        "let mut scope = self.scope.nested()",
        "source.construct(value.value, body.value)",
        "max_bindings: None",
        "max_bindings: Some(limits.max_bindings)",
    ] {
        assert!(
            source.contains(boundary),
            "missing lexical boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_scalar_source_with_scope")
        .unwrap()
        .1;
    for boundary in [
        "preflight(expression, limits.source)",
        "preflight_names(expression)",
        "PureTypeError::InvalidScope",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
    ] {
        assert!(public.find(boundary).unwrap() < public.find("let output = Builder").unwrap());
    }
    let scope = source
        .split_once("pub struct ScalarSourceScope")
        .unwrap()
        .1
        .split_once("pub(crate) struct BindingSource")
        .unwrap()
        .0;
    assert!(!scope.contains("pub bindings"));
    assert!(!scope.contains("pub fn push"));
    assert!(!scope.contains("pub fn pop"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scalar_binding_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "lower_source_call",
        "evaluate_call_arguments_in_scope",
        "catch_unwind",
        "std::ptr::eq",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn shared_native_binding_source_keeps_explicit_stages_bounds_and_product_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/binding_source.rs"))
            .unwrap();
    for boundary in [
        "pub struct BindingSourceLimits",
        "pub enum BindingSourcePhase",
        "pub enum BindingSourceError<Error>",
        "pub struct BindingSourceForm",
        "pub trait BindingSourceAdapter",
        "type Value;",
        "type Result;",
        "type Error;",
        "metadata: &mut Self::Value",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
        "physical(&value, 1, &mut budget)",
        "budget.check_pending(0, 1)",
        "physical(&body, 1, &mut budget)",
        "physical(&output, 0, &mut budget)",
        "max_lowered_depth == 0",
    ] {
        assert!(
            source.contains(boundary),
            "missing binding boundary {boundary}"
        );
    }
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "Default",
        "std::sync",
        "Clone +",
        "Debug +",
    ] {
        assert!(!source.contains(forbidden), "native coupling {forbidden}");
    }
    let public = source.split_once("pub fn lower_binding_source").unwrap().1;
    let sequence = [
        "preflight(expression, limits.source)",
        "binding_source(expression)",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
        ".lower_value(source.binding())",
        "physical(&value, 1, &mut budget)",
        ".lower_body(&source, &value, &mut metadata)",
        "physical(&body, 1, &mut budget)",
        ".finish(&source, value, metadata, body, result)",
        "physical(&output, 0, &mut budget)",
    ];
    for pair in sequence.windows(2) {
        assert!(public.find(pair[0]).unwrap() < public.find(pair[1]).unwrap());
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    for boundary in [
        "crate::binding_source::lower_binding_source(",
        "struct BindingAdapter",
        "let mut local = self.scope.nested()",
        "metadata.members.take()",
        "metadata.group_member_count.is_none_or",
        "source.construct(value, body)",
        "source.span()",
        "crate::function_flow::bind_result(",
        "computation exceeds its node or nesting limit",
    ] {
        assert!(
            reference.contains(boundary),
            "missing reference binding policy {boundary}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    for boundary in [
        "original_source_metadata_native_buffers_and_phase_order_are_preserved",
        "generated_value_budget_reserves_body_and_stops_body_hook",
        "generated_body_uses_same_aggregate_budget_before_finish_hook",
        "rewritten_finish_output_is_rechecked_without_partial_result",
        "shifted_body_and_rewritten_output_depth_are_checked_at_their_own_phases",
        "whole_cold_source_nodes_and_depth_fail_before_value_lowering",
        "native_unwind_releases_owned_parts_and_guarded_frames_once",
        "all_four_move_only_ir_slots_child_boxes_and_vectors_move_without_clone",
        "parsed_native_result_binding_uses_original_schemas_received_values_and_exact_fuel",
        "native_versions_grants_and_undefined_initializer_names_stop_before_later_phases",
        "reference_pure_atomic_group_and_helper_bindings_keep_canonical_wire_and_authority",
        "reference_cold_type_shadow_group_export_and_capture_flow_fail_closed",
        "check_result(&reply)",
        "prepare_call_in_scope",
        "catch_unwind",
        "std::ptr::eq",
    ] {
        assert!(proof.contains(boundary), "missing binding proof {boundary}");
    }
}

#[test]
fn shared_native_choice_source_requires_explicit_queries_after_cold_bounds() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/choice_source.rs"))
            .unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "PartialEq +",
        "Clone +",
        "Debug +",
        "Send +",
    ] {
        assert!(!source.contains(forbidden), "choice coupling {forbidden}");
    }
    let public = source.split_once("pub fn lower_choice_source").unwrap().1;
    for pair in [
        "preflight(expression, limits)",
        "cold_names(expression)",
        "child(ChoiceSourcePhase::When",
        "preflight_with_budget(&when",
        "boolean(&when, &when_type)",
        "child(ChoiceSourcePhase::Then",
        "physical(&then, 1, &mut budget)",
        "child(ChoiceSourcePhase::Otherwise",
        "physical(&otherwise, 1, &mut budget)",
        "same(&then_type, &otherwise_type)",
    ]
    .windows(2)
    {
        assert!(public.find(pair[0]).unwrap() < public.find(pair[1]).unwrap());
    }
    assert!(source.contains("!matches!(value, ScalarValue::Boolean(_))"));
    assert!(source.contains("drop(when_type)"));
    assert!(source.contains(".check_pending(0, 3)"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("crate::choice_source::lower_choice_source("));
    assert!(!reference.contains("crate::scalar_source::lower_choose_form("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/choice_source.rs")).unwrap();
    for boundary in [
        "all_four_native_slots_and_original_boxes_and_vector_buffers_move_unchanged",
        "every_native_unwind_releases_owned_parts_and_guarded_frames_without_retry",
        "forged_boolean_metadata_cannot_accept_an_integer_literal",
        "std::ptr::eq",
        "catch_unwind",
        "parsed_native_choice_checks_both_schemas_but_prepares_only_selected_call_with_exact_fuel",
        "native_choice_never_confers_future_version_or_cold_branch_grants",
        "evaluate_effects_in_scope",
        "check_result(&ScalarValue::Integer(101))",
    ] {
        assert!(proof.contains(boundary), "missing choice proof {boundary}");
    }
}

#[test]
fn shared_flat_group_source_keeps_cold_output_before_explicit_native_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/group_source.rs")).unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "PartialEq +",
        "Clone +",
        "Debug +",
        "Send +",
    ] {
        assert!(
            !source.contains(forbidden),
            "group source coupling {forbidden}"
        );
    }
    let public = source
        .split_once("pub fn lower_flat_group_source")
        .unwrap()
        .1;
    for pair in [
        "header(expression, limits.max_branches)",
        "preflight(expression, limits.source)",
        "cold_headers(expression, limits.max_branches)",
        "lower(index, argument)",
        "physical(&value, 1, &mut budget)",
        "atomic_candidate(&value)",
        "branches.push(ComputedBranch",
        "admit(index, argument, branch)",
    ]
    .windows(2)
    {
        assert!(public.find(pair[0]).unwrap() < public.find(pair[1]).unwrap());
    }
    assert!(source.contains("arguments.len() - index - 1"));
    assert!(source.contains("argument.name.to_owned()"));
    assert!(!source.contains(".clone()"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computed_group.rs")).unwrap();
    for boundary in [
        "lower_flat_group_source(",
        "operation.result_type() == branch.result_type",
        "lower_repeat_sequence_source(",
        "lower_sequence_source(",
        "lower_all_with",
        "branch.name",
        "effect: *effect",
        "GroupSourceError::DuplicateMember { kind, span: at, .. }",
        "phase: GroupSourcePhase::Lower",
        "arguments.iter().skip(index + 1)",
        "only collect bounded sibling diagnostics",
    ] {
        assert!(
            reference.contains(boundary),
            "missing group reference policy {boundary}"
        );
    }
    assert!(!reference.contains("arguments[index]"));
    let diagnostics =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scope_preflight.rs")).unwrap();
    assert!(
        diagnostics
            .contains("failed_parallel_lowering_collects_only_remaining_sibling_diagnostics")
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    for boundary in [
        "original_named_arguments_move_only_results_and_two_stage_order_are_preserved",
        "aggregate_later_output_is_bounded_before_any_admission_hook",
        "matching_result_observations_cannot_replace_uniform_atomic_operation_admission",
        "native_unwind_releases_all_consumed_parts_and_scopes_once_without_retry",
        "reference_nested_cold_error_indices_never_index_the_root_member_array",
        "parsed_native_groups_preserve_original_rows_prepare_real_values_and_exact_fuel_without_dispatch",
        "missing_later_grants_stale_versions_and_wrong_rows_fail_before_value_preparation",
        "evaluate_effects_in_scope",
        "check_result(&ScalarValue::Integer(101))",
        "std::ptr::eq",
        "catch_unwind",
    ] {
        assert!(
            proof.contains(boundary),
            "missing flat group proof {boundary}"
        );
    }
}

#[test]
fn shared_flat_repeat_source_reserves_complete_output_before_explicit_native_factories() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/repeat_source.rs"))
            .unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "PartialEq +",
        "Clone +",
        "Debug +",
        "Send +",
        ".clone()",
    ] {
        assert!(
            !source.contains(forbidden),
            "repeat source coupling {forbidden}"
        );
    }
    let entry = source
        .split_once("pub fn lower_flat_repeat_source")
        .unwrap()
        .1;
    for pair in [
        "header(expression, limits.max_repetitions)",
        "preflight(expression, limits.source)",
        "cold_headers(expression, limits.max_repetitions)",
        ".lower_body(body)",
        "let expected = shape(&value",
        "let cost = measure_source_cost",
        "cost.nodes,",
        "Vec::with_capacity(count)",
        ".materialize(iteration, body, &branches[0])",
        "actual != expected",
        "actual != cost",
        ".admit(index + 1, body, &branches[0], branch)",
    ]
    .windows(2)
    {
        assert!(entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap());
    }
    for boundary in [
        "checked_mul(count)",
        "checked_add(1)",
        "for iteration in 2..=count",
        "atomic_candidate(&value)",
        "name: \"iteration_1\".into()",
        "format!(\"iteration_{iteration}\")",
        "No native Clone/PartialEq/Debug/serde/Send",
    ] {
        assert!(
            source.contains(boundary),
            "missing repeat boundary {boundary}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computed_group.rs")).unwrap();
    for boundary in [
        "lower_flat_repeat_source(",
        "struct RepeatAdapter",
        "original.value.clone()",
        "candidate.result_type == original.result_type",
        "candidate.value.prepared_atomic_operation() == operation",
        "functions::host_source_extra",
        "finish_flat_group(value, span)",
        "lower_repeat_sequence_source(",
    ] {
        assert!(
            reference.contains(boundary),
            "missing repeat reference policy {boundary}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/repeat_source.rs")).unwrap();
    for boundary in [
        "original_body_result_buffers_and_once_factory_admission_order_are_preserved",
        "complete_shifted_physical_reservation_precedes_every_factory",
        "folded_constructor_and_opaque_source_costs_are_reserved_before_factories",
        "equal_node_count_with_changed_depth_cannot_reuse_the_reserved_shape",
        "same_sized_native_result_observations_do_not_become_declarations",
        "native_unwind_drops_all_consumed_instances_and_restores_scopes_once",
        "late_native_cost_failure_and_unwind_drop_every_instance_without_admission",
        "parsed_native_repeat_prepares_original_schema_values_and_exact_fuel_without_dispatch",
        "wrong_copy_declarations_changed_values_and_stale_live_policy_cannot_return_partial_requests",
        "evaluate_effects_in_scope",
        "std::ptr::eq",
        "check_result(&ScalarValue::Integer(101))",
    ] {
        assert!(proof.contains(boundary), "missing repeat proof {boundary}");
    }
}

#[test]
fn shared_owned_sequence_source_moves_native_children_without_effect_round_trips() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/sequence_source.rs"))
            .unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "Clone +",
        "PartialEq +",
        "Debug +",
        "Send +",
        ".clone()",
    ] {
        assert!(
            !source.contains(forbidden),
            "sequence source coupling {forbidden}"
        );
    }
    let entry = source.split_once("pub fn lower_sequence_source").unwrap().1;
    for pair in [
        "header(expression, limits.max_branches)",
        "preflight(expression, limits.source)",
        "cold_headers(expression, limits.max_branches)",
        "lower(index, argument)",
        "members.is_empty()",
        "joined_member_name_fits(&argument.name, &branch.name)",
        "physical(&branch.value, 1, &mut budget)",
        "atomic_candidate(&branch.value)",
        "admit(index, member, argument, branch)",
    ]
    .windows(2)
    {
        assert!(entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap());
    }
    for boundary in [
        "SequenceSourceMember::Sequence { branches } if nested",
        "branches.push(branch)",
        "origins.push((index, member))",
        "previous.name == branch.name",
        "arguments.len() - index - 1",
        "never formatted or exposed in source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing sequence boundary {boundary}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computed_group.rs")).unwrap();
    let sequence = reference
        .split_once("fn lower_sequence_nested")
        .unwrap()
        .1
        .split_once("fn lower_flat")
        .unwrap()
        .0;
    assert!(sequence.contains("lower_sequence_source("));
    assert!(!sequence.contains("into_effect("));
    assert!(!sequence.contains(".clone()"));
    assert!(!sequence.contains("control_flow::lower_sequence_with"));
    assert!(sequence.contains("Effect::Sequence { steps }"));
    assert!(sequence.contains("operation.result_type() == branch.result_type"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/sequence_source.rs")).unwrap();
    for boundary in [
        "original_rows_order_labels_and_move_only_results_survive_flattening",
        "whole_shifted_output_reserves_future_roots_before_later_lowering",
        "joined_names_are_bounded_before_allocation_and_collisions_before_admission",
        "native_unwind_drops_all_consumed_rows_once_and_never_retries",
        "reference_nested_sequences_keep_canonical_wire_authority_and_diagnostics",
    ] {
        assert!(
            proof.contains(boundary),
            "missing owned sequence proof {boundary}"
        );
    }
    let host =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    assert!(host.contains(
        "parsed_nested_sequence_preserves_two_native_schemas_values_fuel_and_live_rejection"
    ));
    assert!(host.contains("assert_eq!(fuel.remaining(), 93)"));
    assert!(host.contains("compile_sequence(&argument.value, host)"));
}

#[test]
fn shared_repeat_sequence_source_reserves_whole_forests_before_native_factories_and_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/repeat_sequence_source.rs"),
    )
    .unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "Clone +",
        "PartialEq +",
        "Debug +",
        "Send +",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
    ] {
        assert!(
            !source.contains(forbidden),
            "repeat-sequence coupling {forbidden}"
        );
    }
    let entry = source
        .split_once("pub fn lower_repeat_sequence_source")
        .unwrap()
        .1;
    for pair in [
        "header(expression, bounds.max_repetitions)",
        "preflight(expression, bounds.source)",
        "cold_headers(expression, bounds.max_repetitions)",
        "group_headers(expression, limits.max_branches)",
        ".lower_sequence(body)",
        "let expanded_width = width",
        "expected.nodes - 1",
        "joined_member_name_fits(&prefix, &branch.name)",
        "let cost = measure_source_cost",
        "cost.nodes - 1",
        "Vec::with_capacity(count)",
        ".materialize_sequence(iteration, body, original)",
        "actual != expected",
        "branch.name != original.name",
        "actual != cost",
        ".admit_member(index + 1, member, body, original, candidate)",
        "Vec::with_capacity(expanded_width)",
    ]
    .windows(2)
    {
        assert!(
            entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap(),
            "incorrect repeat-sequence order {pair:?}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computed_group.rs")).unwrap();
    assert!(reference.contains("lower_repeat_sequence_source("));
    assert!(reference.contains("Ok(original.to_vec())"));
    assert!(reference.contains("sequence_branches(value, body.span)"));
    assert!(!reference.contains("control_flow::lower_repeat_with"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/repeat_sequence_source.rs"))
            .unwrap();
    for boundary in [
        "original_template_rows_move_once_and_factories_precede_all_ordered_admission",
        "all_four_native_slots_boxes_and_operand_buffers_require_no_clone_debug_serde_or_send",
        "flattened_physical_reservation_does_not_recharge_removed_group_roots",
        "opaque_source_reservation_is_complete_before_any_instance_factory",
        "changed_width_labels_shape_cost_or_candidate_stops_later_factories_and_all_admission",
        "native_unwind_releases_consumed_instances_and_restores_frames_without_replay",
        "reference_nested_repeat_names_wire_authority_and_expansion_diagnostics_remain_compatible",
    ] {
        assert!(
            proof.contains(boundary),
            "missing repeat-sequence proof {boundary}"
        );
    }
    let host =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    assert!(host.contains("parsed_repeated_sequence_preserves_native_rows_values_exact_fuel_and_rejects_changed_semantics"));
    assert!(host.contains("assert_eq!(fuel.remaining(), 87)"));
    assert!(host.contains("changed native argument semantics"));
}

#[test]
fn shared_sequence_sessions_gate_each_request_on_the_original_accepted_reply_without_product_state()
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/sequence_evaluation.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        "Identity: Clone",
        "Declaration: Clone",
        "Request: Clone",
        "Node: Clone",
        "IrResult: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "standalone sequence session gained {forbidden}"
        );
    }
    for required in [
        "SequenceEvaluationEnvironment",
        "PreparedSequenceMember",
        "PendingReply::new",
        "SequenceSession",
        "SequenceEvaluationLimits",
        "SequenceStatus",
        "SequenceEnd::HostUncertain",
        "SequenceEnd::Cancelled",
        "SequenceEnd::Completed",
        "SequenceEnd::Failed",
        "Reply: Borrow<View>",
        "Declaration: HostResultDomain<View>",
        "Identity: PartialEq",
        "No native Clone/Debug/serde/Send/Sync bound",
        "no retry, fuel refund or automatic restoration",
    ] {
        assert!(
            source.contains(required),
            "sequence session lost {required}"
        );
    }
    let start = source
        .split_once("pub fn start<Environment>")
        .unwrap()
        .1
        .split_once("pub fn poll<Environment>")
        .unwrap()
        .0;
    let shape = source
        .split_once("pub(crate) fn admit_sequence_shape")
        .unwrap()
        .1
        .split_once("pub fn start<Environment>")
        .unwrap()
        .0;
    let physical = shape.find("physical(").unwrap();
    let candidates = shape.find("atomic_candidate(&branch.value)").unwrap();
    assert!(physical < candidates);
    let shape = start
        .find("admit_sequence_shape(expression, limits)")
        .unwrap();
    let cold = start.find(".preflight_member(index, branch)").unwrap();
    let charge = start.find("fuel.charge(1)").unwrap();
    assert!(shape < cold && cold < charge);
    let poll = source
        .split_once("pub fn poll<Environment>")
        .unwrap()
        .1
        .split_once("pub fn status")
        .unwrap()
        .0;
    let compact = poll.split_whitespace().collect::<String>();
    assert!(
        compact.find("self.core.publish_with(").unwrap()
            < compact.find("self.fuel.charge(1)").unwrap()
    );
    assert!(
        compact.find("self.fuel.charge(1)").unwrap()
            < compact.find(".prepare_member(index,branch,").unwrap()
    );
    let publish = source
        .split_once("pub(crate) fn publish_with<")
        .unwrap()
        .1
        .split_once("pub(crate) fn try_accept<")
        .unwrap()
        .0;
    let close = publish
        .find("self.state = State::Terminal(SequenceEnd::HostUncertain)")
        .unwrap();
    let prepare = publish.find("prepare(self.next,").unwrap();
    let waiting = publish.find("PendingReply::new(").unwrap();
    assert!(close < prepare && prepare < waiting);
    assert!(publish.find("SequencePoll::Awaiting").unwrap() < close);
    let receive = source
        .split_once("pub(crate) fn try_accept<")
        .unwrap()
        .1
        .split_once("pub(crate) fn cancel")
        .unwrap()
        .0;
    assert!(
        receive.find("std::mem::replace").unwrap() < receive.find("pending.try_accept").unwrap()
    );
    assert!(receive.find("pending.try_accept").unwrap() < receive.find("self.next += 1").unwrap());
    assert!(!receive.contains("prepare_member"));
    let cancel = source.split_once("pub(crate) fn cancel").unwrap().1;
    assert!(cancel.find("std::mem::replace").unwrap() < cancel.find("drop(old)").unwrap());
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/sequence_evaluation.rs"))
            .unwrap();
    for proof in [
        "cold_shape_limits_and_candidates_precede_all_native_hooks",
        "move_only_native_rows_replies_and_requests_preserve_pointers_without_eager_work",
        "rejected_identity_authority_type_and_value_preserve_input_and_pending_member",
        "preparation_failure_or_unwind_is_terminal_without_fuel_refund_or_retry",
        "every_reply_callback_unwind_releases_identity_and_closes_without_rearming",
        "unsized_native_domain_and_text_reply_view_need_no_cloning_or_serialization",
    ] {
        assert!(tests.contains(proof), "sequence lifecycle lost {proof}");
    }
    let native =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    for proof in [
        "parsed_nested_repeat_runs_one_native_request_per_accepted_reply_with_exact_fuel",
        "parsed_sequence_revocation_cancellation_and_stale_replies_never_invoke_a_successor",
        "NativeCounters",
        ".invoke(request)",
        "session.fuel_remaining(), 85",
        "(counters.left, counters.right), (85, 22)",
        "SequenceEnd::Cancelled",
        "SequenceEnd::Failed",
    ] {
        assert!(
            native.contains(proof),
            "parsed native sequence lost {proof}"
        );
    }
}

#[test]
fn catalog_flat_group_compilation_reuses_shared_scopes_lowering_and_full_flow_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_group_source.rs"))
            .unwrap();
    for forbidden in [
        "leselang_vm",
        "leserpent_domain",
        "rusqlite",
        "serde::",
        "unsafe",
        ".unwrap(",
        ".expect(",
        "panic!",
        "apply_binary(",
    ] {
        assert!(
            !source.contains(forbidden),
            "group compiler gained {forbidden}"
        );
    }
    let hook = source
        .split_once("pub trait CatalogGroupSourceEnvironment")
        .unwrap()
        .1
        .split_once("pub enum CatalogGroupSourceError")
        .unwrap()
        .0;
    for forbidden in ["fn lower", "FnOnce", "FnMut", "Clone +", "Send +", "serde"] {
        assert!(
            !hook.contains(forbidden),
            "group source adapter gained {forbidden}"
        );
    }
    for boundary in [
        "for<'expression> GroupFlowEnvironment",
        "source_group_branch",
        "Compiler::new",
        ".inner",
        ".lower_leaf(source, scope)",
        "lower_flat_group_source(",
        "infer_call_flow_type(",
        "infer_group_flow_type(",
        "group_branch_type_matches",
        "CatalogGroupSourceError::GroupLimit",
        "CatalogGroupSourceError::NestedGroup",
        "CatalogGroupSourceError::NonAtomic",
        "Self::Error::InconsistentType",
    ] {
        assert!(source.contains(boundary), "group compiler lost {boundary}");
    }
    let entry = source
        .split_once("pub fn compile_catalog_group_source")
        .unwrap()
        .1;
    let lexical = entry.find("preflight_closed_control_source").unwrap();
    let groups = entry.find("cold_groups(source, limits)").unwrap();
    let catalog = entry.find("cold_catalog_profile(").unwrap();
    let lower = entry.find("lower_preflighted_control_source(").unwrap();
    assert!(lexical < groups && groups < catalog && catalog < lower);
    let scalar =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/scalar_source.rs"))
            .unwrap();
    assert!(scalar.contains("preflight_binding_scope(expression, scope, max_bindings, false)"));
    assert!(scalar.contains("preflight_binding_scope(expression, scope, max_bindings, true)"));
    let legacy =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_source.rs"))
            .unwrap();
    assert!(legacy.contains("cold_catalog_profile(source, host, limits, false, local_call)"));
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for proof in [
        "complete_group_control_expression_preserves_native_declarations_without_slot_copies",
        "all_cold_catalog_signatures_versions_and_grants_precede_group_slot_mapping",
        "group_call_and_whole_expression_limits_are_inclusive_without_per_leaf_reset",
        "closed_reference_preflight_reuses_loop_fold_and_bind_scopes_without_native_alias_fallback",
        "native_branch_error_or_unwind_releases_owned_prefix_once_without_partial_output",
        "group_profile_is_additive_and_original_call_only_compiler_still_rejects_groups",
    ] {
        assert!(tests.contains(proof));
    }
    let vertical =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_reply_reentry.rs"))
            .unwrap();
    let program = vertical
        .split_once("fn program(")
        .unwrap()
        .1
        .split_once("struct Host")
        .unwrap()
        .0;
    assert!(program.contains("compile_catalog_group_source("));
    assert!(!program.contains("lower_flat_group_source("));
    assert!(!program.contains("Node::Bind"));
}

#[test]
fn complete_group_reply_assembly_is_native_free_and_keeps_cold_checks_before_owned_fuel() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/group_reply.rs")).unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        ".expect(",
        ".unwrap(",
        "panic!",
        "Identity: Clone",
        "Reply: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "group reply gained {forbidden}"
        );
    }
    for boundary in [
        "Declaration: ?Sized",
        "admit: impl FnOnce",
        "GroupReplyError::Count",
        "GroupReplyError::Row",
        "std::ptr::eq",
        "lookup_native_group_member",
        "into_members",
        "not a receipt",
        "not group-value projection",
    ] {
        assert!(source.contains(boundary), "group reply lost {boundary}");
    }
    let assemble = source
        .split_once("pub fn assemble_group_reply")
        .unwrap()
        .1
        .split_once("pub fn restore_group_reply")
        .unwrap()
        .0;
    let cold = assemble
        .find("check_group_reply(group, &accepted, limits)?")
        .unwrap();
    let debit = assemble.find("fuel.charge(").unwrap();
    let native = assemble.find("admit(&reply, fuel)").unwrap();
    assert!(cold < debit && debit < native);
    let check = source.split_once("fn check_group_reply<").unwrap().1;
    let physical = check.find("physical(").unwrap();
    let candidate = check.find("atomic_candidate").unwrap();
    let count = check.find("accepted.len() != branches.len()").unwrap();
    let row = check.find("std::ptr::eq").unwrap();
    assert!(physical < candidate && candidate < count && count < row);
    assert!(!check.contains("fuel.charge("));
    assert!(!check.contains("admit("));
    for path in ["sequence_evaluation", "parallel_evaluation"] {
        let session =
            std::fs::read_to_string(root.join(format!("crates/leselang-hir-core/src/{path}.rs")))
                .unwrap();
        assert!(session.contains("pub fn into_completed_fuel(self) -> Result<Fuel, Self>"));
        assert!(session.contains("Ok(self.fuel)"));
        assert!(session.contains("Err(self)"));
    }
    let component =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_reply.rs")).unwrap();
    for proof in [
        "partial_reordered_foreign_and_duplicate_rows_are_cold_rejections",
        "mixed_generation_changed_values_and_revoked_authority_require_fresh_whole_admission",
        "native_failure_and_unwind_consume_inputs_once_without_refund_or_partial_output",
        "original_parallel_fuel_moves_only_after_complete_result_handoff",
    ] {
        assert!(component.contains(proof));
    }
    let vertical =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_reply_reentry.rs"))
            .unwrap();
    for proof in [
        "parsed_parallel_results_bind_project_and_reenter_original_enclosing_control_with_one_meter",
        "aggregate_data_does_not_bypass_fresh_outer_acceptance_or_reentry_authority",
        "selected_member_never_falls_back_to_another_operation_or_stale_native_view",
        "session.into_completed_fuel()",
        "PendingReply::new",
        "fuel.remaining(), 77",
    ] {
        assert!(vertical.contains(proof));
    }
    assert!(!vertical.contains("#![cfg(feature = \"leserpent-profile\")]"));
}

#[test]
fn native_parallel_sessions_share_atomic_adapters_and_close_the_complete_reply_barrier() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/parallel_evaluation.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        ".expect(",
        ".unwrap(",
        "panic!",
        "Identity: Clone",
        "Reply: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "native parallel session gained {forbidden}"
        );
    }
    for boundary in [
        "SequenceEvaluationEnvironment as ParallelEvaluationEnvironment",
        "SequenceAccepted as ParallelAccepted",
        "PendingReply::new",
        "DuplicateIdentity",
        "ParallelStatus::ReadyResults",
        "ParallelPoll::Results",
        "ParallelEnd::HostUncertain",
        "ParallelEnd::Cancelled",
        "ParallelEnd::Failed",
        "Reply: Borrow<View>",
        "Declaration: HostResultDomain<View>",
        "partial result",
        "NOT atomic external dispatch",
    ] {
        assert!(
            source.contains(boundary),
            "missing native parallel boundary {boundary}"
        );
    }
    let start = source
        .split_once("pub fn start<Environment>")
        .unwrap()
        .1
        .split_once("pub fn poll<Environment>")
        .unwrap()
        .0;
    let shape = source
        .split_once("pub(crate) fn admit_parallel_shape")
        .unwrap()
        .1
        .split_once("pub fn start<Environment>")
        .unwrap()
        .0;
    assert!(shape.find("physical(").unwrap() < shape.find("atomic_candidate").unwrap());
    assert!(
        start.find("admit_parallel_shape(").unwrap() < start.find(".preflight_member").unwrap()
    );
    assert!(start.find(".preflight_member").unwrap() < start.find("fuel.charge(1)").unwrap());
    let receive = source
        .split_once("pub(crate) fn try_accept<")
        .unwrap()
        .1
        .split_once("pub(crate) fn cancel")
        .unwrap()
        .0;
    assert!(
        receive.find("members.get(index)").unwrap() < receive.find("std::mem::replace").unwrap()
    );
    assert!(
        receive.find("std::mem::replace").unwrap() < receive.find("pending.try_accept").unwrap()
    );
    assert!(!receive.contains("prepare_member"));
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/parallel_evaluation.rs"))
            .unwrap();
    for proof in [
        "out_of_order_replies_publish_only_one_complete_ordered_original_result_batch",
        "duplicate_identity_refuses_the_complete_batch_before_publication",
        "preparation_failure_or_fuel_exhaustion_never_publish_a_partial_batch_or_retry",
        "every_reply_callback_unwind_discards_sibling_barrier_and_never_rearms",
        "unsized_native_result_domains_and_borrowed_text_views_need_no_native_clones",
        "maximum_width_and_inclusive_physical_limits_keep_one_owned_fuel_meter",
    ] {
        assert!(
            tests.contains(proof),
            "missing native parallel proof {proof}"
        );
    }
    let vertical =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    assert!(
        vertical
            .contains("parsed_parallel_batch_runs_real_calls_and_closes_an_ordered_reply_barrier")
    );
    assert!(vertical.contains("missing_later_grant"));
}

#[test]
fn shared_accepted_binding_reentry_preserves_original_sites_and_validation_order_without_product_state()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_reentry.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        "Identity: Clone",
        "Capture: Clone",
        "Reply: Clone",
        "Node: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "binding reentry gained {forbidden}"
        );
    }
    for required in [
        "EffectContinuation",
        "pub(crate) fn new",
        "AcceptedReply",
        "Declaration: ?Sized",
        "EffectReentryEnvironment",
        "ResumableEffectOutcome",
        "resume_accepted_effects",
        "preflight_value_scope",
    ] {
        assert!(source.contains(required), "binding reentry lost {required}");
    }
    let resume = source
        .split_once("pub fn resume_accepted_effects<")
        .unwrap()
        .1;
    let physical = resume.find("preflight(body,").unwrap();
    let cold = resume.find(".preflight_effect(effect)").unwrap();
    let restore = resume.find(".restore_capture(").unwrap();
    let restored = source
        .split_once("pub(crate) fn restore_effect_scope")
        .unwrap()
        .1
        .split_once("pub(crate) fn bind_effect_value")
        .unwrap()
        .0;
    let quota = restored
        .find("scope.len() >= limits.pure.max_bindings")
        .unwrap();
    let charge = restored.find("fuel.charge(scope.len() as u64)").unwrap();
    let prefix = restored.find("preflight_value_scope(&scope,").unwrap();
    let shadow = restored.find("scope.get(name)").unwrap();
    let projection = resume.find(".bind_reply(").unwrap();
    let binding = source
        .split_once("pub(crate) fn bind_effect_value")
        .unwrap()
        .1;
    let insert = binding.find(".push(name, value)").unwrap();
    let mapped = binding.find("preflight_value_scope(scope,").unwrap();
    let body = resume.rfind("evaluate(").unwrap();
    assert!(physical < cold && cold < restore && quota < charge && charge < prefix);
    assert!(prefix < shadow && insert < mapped);
    assert!(resume.find("restore_effect_scope(").unwrap() < projection);
    assert!(projection < resume.find("bind_effect_value(").unwrap());
    assert!(resume.find("bind_effect_value(").unwrap() < body);
    let walker =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_evaluation.rs"))
            .unwrap();
    assert!(walker.contains("EffectContinuation::new(name, body, capture)"));
    assert!(walker.contains(".map(ResumableEffectOutcome::into_legacy)"));
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_evaluation.rs"))
            .unwrap();
    for proof in [
        "shared_accepted_reentry_preserves_alias_identity_exact_fuel_and_original_binding_sites",
        "native_capture_metadata_cannot_redirect_the_core_owned_reply_binding_or_body",
        "projection_unwind_releases_restored_aliases_and_actual_reply_without_rearming",
        "reentry_bounds_and_current_cold_schemas_stop_before_restore_projection_and_fuel",
        "invalid_restored_prefixes_stop_before_reply_projection_or_body_execution",
        "restoration_and_projection_failure_or_unwind_never_rearm_or_enter_the_body",
        "restoration_prefix_fuel_is_charged_before_projection_without_refund",
        "shared_gui_reentry_accepts_a_move_only_text_payload_and_unsized_native_domain",
        "cancelled_core_owned_continuation_never_restores_or_projects_late_input",
    ] {
        assert!(tests.contains(proof), "binding reentry lost proof {proof}");
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    for proof in [
        "parsed_binding_invokes_native_calls_accepts_original_domains_and_reenters_with_exact_fuel",
        "execution.invoke(request)",
        "execution.actual.get(), 42",
        "fuel.remaining(), 94",
    ] {
        assert!(parsed.contains(proof), "parsed binding chain lost {proof}");
    }
}

#[test]
fn borrowed_control_session_keeps_reply_ready_and_owned_fuel_without_product_state() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_session.rs"))
            .unwrap();
    let production = source
        .split("\n#[cfg(test)]\n")
        .next()
        .unwrap()
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        ".unwrap(",
        "Identity: Clone",
        "Capture: Clone",
        "Reply: Clone",
        "Node: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "control session gained {forbidden}"
        );
    }
    for required in [
        "ControlSession",
        "EffectSessionEnvironment",
        "CorrelatedEffectRequest",
        "PendingReply::new",
        "EffectSessionEnd::HostUncertain",
        "EffectSessionEnd::Cancelled",
        "EffectSessionEnd::Failed",
        "EffectSessionEnd::Completed",
        "AcceptedBinding",
        "AcceptedFinal",
        "Reply: Borrow<View>",
        "Declaration: HostResultDomain<View>",
        "Identity: PartialEq",
        "type Declaration: ?Sized",
    ] {
        assert!(source.contains(required), "control session lost {required}");
    }
    let poll = source.split_once("pub(crate) fn poll_with<").unwrap().1;
    let close = poll.find("std::mem::replace").unwrap();
    assert!(poll.find("EffectSessionPoll::Awaiting").unwrap() < close);
    assert!(close < poll.find("with_owned_scope_state(").unwrap());
    assert!(close < poll.find("resume(accepted,").unwrap());
    let legacy = source.split_once("pub fn poll<Environment>").unwrap().1;
    assert!(legacy.contains("self.core.poll_with("));
    assert!(legacy.contains("evaluate_resumable_effects_in_scope("));
    assert!(legacy.contains("resume_accepted_effects("));
    assert!(
        poll.find("with_owned_scope_state(").unwrap()
            < poll.find("self.state = State::AwaitingFinal").unwrap()
    );
    assert!(
        poll.find("drop(identity)").unwrap() < poll.find("EffectSessionEnd::Completed").unwrap()
    );
    let receive = source
        .split_once("pub(crate) fn try_accept<")
        .unwrap()
        .1
        .split_once("impl<Node:")
        .unwrap()
        .0;
    assert!(
        receive.find("std::mem::replace").unwrap() < receive.find("pending.try_accept").unwrap()
    );
    assert!(receive.contains("State::AcceptedBinding(accepted)"));
    assert!(receive.contains("State::AcceptedFinal(accepted)"));
    for forbidden in [
        "resume_accepted_effects(",
        "correlate_request(",
        "bind_reply(",
        "fuel.charge",
    ] {
        assert!(
            !receive.contains(forbidden),
            "reply acceptance implicitly ran {forbidden}"
        );
    }
    let cancel = source
        .split_once("pub(crate) fn cancel")
        .unwrap()
        .1
        .split_once("pub(crate) fn try_accept")
        .unwrap()
        .0;
    assert!(cancel.find("std::mem::replace").unwrap() < cancel.find("drop(old)").unwrap());
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "accepted_binding_waits_for_explicit_poll_and_returns_requests_and_pure_tail_once",
        "final_native_reply_is_projected_only_on_later_poll_and_handed_off_once",
        "wrong_identity_policy_type_and_domain_return_exact_input_and_keep_waiting",
        "cancel_initial_waiting_or_accepted_binding_and_final_reply_prevents_all_later_work",
        "physical_prefix_limits_and_cold_policy_precede_fuel_and_preparation",
        "native_preparation_capture_and_correlation_failures_or_unwind_are_terminal",
        "every_native_acceptance_callback_unwind_seals_the_session_without_rearming",
        "restored_binding_or_final_projection_failure_unwind_and_bounds_never_replay",
        "owned_initial_prefix_cleanup_unwind_cannot_install_a_waiting_request",
        "cancellation_cleanup_unwind_retains_cancelled_reason_and_drops_once",
        "accepted_identity_or_capture_cleanup_unwind_cannot_publish_completion_or_successor",
        "correlation_error_followed_by_capture_cleanup_unwind_stays_host_uncertain",
        "exhaustion_is_terminal_and_never_refills_while_waiting_or_after_acceptance",
    ] {
        assert!(tests.contains(proof), "control session lost {proof}");
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    assert!(
        parsed.contains(
            "parsed_binding_session_owns_reply_gated_reentry_final_output_and_cancellation"
        )
    );
    assert!(parsed.contains("cancelled.fuel_remaining(), 98"));
    assert!(parsed.contains("execution.invoke(write)"));
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_evaluation.rs"))
        .unwrap();
    assert!(gui.contains("gui_local_control_session_owns_borrowed_dispatch_move_only_input_and_cancel_after_acceptance"));
}

#[test]
fn recursive_control_source_is_borrowed_once_admitted_and_product_free() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/control_source.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        ".unwrap(",
        "Type: Clone",
        "Type: PartialEq",
        "Type: std::fmt::Debug",
    ] {
        assert!(
            !production.contains(forbidden),
            "source entry gained {forbidden}"
        );
    }
    for required in [
        "ControlSourceAdapter",
        "ControlSourceScope",
        "prefix.to_vec()",
        "&ty",
        "future + 1",
        "future + 2",
        "check_pending(0, future)",
        "physical(&value, depth",
        "preflight_with_budget(&when",
        "drop(ty)",
        "drop(otherwise_type)",
        "ControlSourcePhase::Admit",
    ] {
        assert!(source.contains(required), "source entry lost {required}");
    }
    let entry = source.split_once("pub fn lower_control_source").unwrap().1;
    let build = entry.find(".build(expression").unwrap();
    for preflight in [
        "preflight(expression",
        "preflight_names(expression",
        "cold_names(expression",
        "preflight_bindings(",
        "crate::projection_source::preflight_names(expression)",
    ] {
        assert!(entry.find(preflight).unwrap() < build);
    }
    assert!(build < entry.find(".admit(").unwrap());
    assert_eq!(production.matches(".admit(").count(), 1);
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/control_source.rs")).unwrap();
    for proof in [
        "recursive_bind_choose_borrows_original_scope_metadata_and_moves_native_buffers",
        "whole_cold_signatures_and_lexical_errors_stop_every_native_hook",
        "prefix_names_and_active_quota_are_checked_before_native_work",
        "initializer_sees_parent_and_sibling_scopes_do_not_leak",
        "aggregate_leaf_expansion_retains_outer_future_roots_before_sibling_hooks",
        "shifted_native_leaf_depth_is_not_reset_at_each_control_boundary",
        "independent_condition_purity_and_literal_kind_cannot_be_overridden",
        "every_native_failure_or_unwind_releases_owned_parts_without_admission_or_retry",
        "native_observation_cleanup_unwind_cannot_publish_a_complete_tree",
    ] {
        assert!(tests.contains(proof), "source entry lost {proof}");
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    assert!(
        parsed.contains("parsed_recursive_control_source_runs_selected_native_calls_and_pure_tail")
    );
    assert!(
        parsed.contains(
            "recursive_control_native_admission_rejects_cold_types_and_unfinished_capture"
        )
    );
    assert!(parsed.contains("session.fuel_remaining(), 84"));
    assert!(parsed.contains("other.fuel_remaining(), 86"));
    assert!(parsed.contains("infer_call_flow_type("));
}

#[test]
fn bound_native_projection_source_has_exact_borrowed_type_queries_without_product_frames() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/bound_projection_source.rs"),
    )
    .unwrap();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .unwrap()
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        ".expect(",
        ".unwrap(",
        "Result: Clone",
        "Result: PartialEq",
        "lower_value(",
        "unsafe",
    ] {
        assert!(
            !production.contains(forbidden),
            "bound projection gained {forbidden}"
        );
    }
    for required in [
        "BoundProjectionSourceEnvironment",
        "BoundProjectionSourceLimits",
        "BoundProjectionSourceError::Unbound",
        "BoundProjectionSourceError::BoundReference",
        "PureType::Result(result)",
        "ProjectionSourceError::NonResult",
        "environment.field(result",
        "environment.member(result",
        "member.construct(operation)",
        "Node::Local",
        "budget.visit(1, 0, 0)",
    ] {
        let compact = source.split_whitespace().collect::<Vec<_>>().join(" ");
        let flat = compact
            .replace("environment .", "environment.")
            .replace("budget .", "budget.");
        assert!(flat.contains(required), "bound projection lost {required}");
    }
    let entry = source
        .split_once("pub fn lower_bound_projection_source")
        .unwrap()
        .1;
    let query = entry.find("environment").unwrap_or(entry.len());
    assert!(query < entry.len());
    for gate in [
        "preflight(expression",
        "preflight_names(expression",
        "prefix.len()",
        "PureType::Result(result)",
    ] {
        assert!(entry.find(gate).unwrap() < entry.find(".member(").unwrap());
        assert!(entry.find(gate).unwrap() < entry.find(".field(").unwrap());
    }
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/bound_projection_source.rs"))
            .unwrap();
    for proof in [
        "two_unrelated_schemas_borrow_original_result_metadata_and_export_all_six_scalars",
        "bound_group_member_uses_exact_group_observation_and_moves_returned_slots",
        "missing_scalar_foreign_and_non_group_bindings_cannot_launder_exports_by_name",
        "malformed_cold_projection_metadata_precedes_all_native_queries",
        "prefix_validity_quota_and_all_minimum_source_output_limits_precede_native_queries",
        "inclusive_field_two_node_one_depth_and_member_one_node_zero_depth_bounds_are_exact",
        "native_error_and_unwind_do_not_copy_drop_or_requery_borrowed_metadata",
    ] {
        assert!(tests.contains(proof), "bound projection lost {proof}");
    }
    assert!(source.contains("lower_projection(expression, prefix, limits, environment, false)"));
    assert!(source.contains("lower_projection(expression, prefix, limits, environment, true)"));
    assert!(source.contains("allow_member_field && callee == \"field\" && child == \"member\""));
    for proof in [
        "catalog_member_field_composition_keeps_direct_entry_closed_and_moves_native_slots",
        "catalog_member_field_bounds_and_cold_metadata_precede_native_export_queries",
        "catalog_member_field_refusal_and_unwind_stop_without_copying_native_errors",
    ] {
        assert!(source.contains(proof));
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/bound_projection_session.rs"))
            .unwrap();
    for proof in [
        "parsed_record_reply_fields_drive_native_branch_arguments_with_exact_fuel_and_no_snapshot_copy",
        "foreign_record_domain_and_cancellation_before_field_projection_preserve_native_reply_ownership",
        "whole_native_source_typing_rejects_foreign_fields_unbound_aliases_and_cold_grants_before_calls",
        "post_acceptance_live_policy_and_native_view_identity_fail_before_write_without_retry",
        "weak.strong_count()",
        "session.fuel_remaining()",
        "compile_catalog_source",
        "CatalogSourceEnvironment",
    ] {
        assert!(parsed.contains(proof));
    }
}

#[test]
fn shared_scalar_loop_source_reuses_original_ir_scope_limits_and_reference_gates() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/scalar_source.rs"))
            .unwrap();
    for boundary in [
        "pub(crate) struct LoopSource",
        "pub(crate) fn loop_source",
        "pub(crate) fn lower_loop_body",
        "pub(crate) fn scalar_operand",
        "*value > MAX_LOOP_ITERATIONS",
        "source.limit()?",
        "preflight_binding_scope(source.condition, &mut local, max_bindings, closed)",
        "preflight_binding_scope(source.next, &mut local, max_bindings, closed)",
        "source.construct(initial, body, limit)",
        "if ty != state_type",
    ] {
        assert!(
            source.contains(boundary),
            "missing loop boundary {boundary}"
        );
    }
    assert!(!source.contains("LoopBudget::new"));
    assert!(!source.contains("0..limit"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let loop_lowerer = reference
        .split_once("fn lower_loop")
        .unwrap()
        .1
        .split_once("fn lower_fold")
        .unwrap()
        .0;
    assert!(loop_lowerer.contains("crate::loop_source::bind_loop_source(arguments, span)"));
    assert!(loop_lowerer.contains(".prepare(|child|"));
    assert!(loop_lowerer.contains(".lower_body(|child|"));
    assert!(!loop_lowerer.contains("Computation::Loop {"));
    assert!(!loop_lowerer.contains("next_type != state_type"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scalar_loop_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "evaluate_call_arguments_in_scope",
        "LoopError::IterationLimit",
        "PureEvaluationFault::FuelExhausted",
        "catch_unwind",
        "std::ptr::eq",
        "serde_json::to_vec",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn shared_scalar_fold_source_keeps_two_local_policy_without_collection_execution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/scalar_source.rs"))
            .unwrap();
    for boundary in [
        "pub(crate) use fold::{FoldSource, fold_items, fold_next, fold_source}",
        "preflight_binding_scope(source.items, scope, max_bindings, closed)",
        "source.check_scope(&self.scope, Some(max_bindings))",
        "fold_items(items, span(source.items))",
        "fold_next(next, state_type, span(source.next))",
        "source.construct(items, initial, next, limit)",
    ] {
        assert!(
            source.contains(boundary),
            "missing fold boundary {boundary}"
        );
    }
    let fold =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/scalar_source/fold.rs"))
            .unwrap();
    for boundary in [
        "pub(crate) struct FoldSource",
        "pub item: &'source str",
        "pub fn check_scope<Value, Error>",
        "scope.len().checked_add(2)",
        "MAX_STRING_LIST_ITEMS as u64",
        "pub(crate) fn fold_source",
        "pub(crate) fn fold_items",
        "pub(crate) fn fold_next",
        "Computation::Fold {",
        "if ty != ScalarType::StringList",
        "if ty != state_type",
    ] {
        assert!(fold.contains(boundary), "missing fold boundary {boundary}");
    }
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "FoldCursor",
        "Fuel::",
        "0..limit",
    ] {
        assert!(
            !fold.contains(coupling),
            "unexpected fold coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let lowerer = reference
        .split_once("fn lower_fold")
        .unwrap()
        .1
        .split_once("pub(super) fn valid_local")
        .unwrap()
        .0;
    for boundary in [
        "crate::fold_source::bind_fold_source(arguments, span)",
        ".prepare_items(",
        ".prepare_initial(",
        ".lower_next(",
        ".finish()",
    ] {
        assert!(lowerer.contains(boundary));
    }
    assert!(!lowerer.contains("Computation::Fold {"));
    assert!(!reference.contains("fn scalar("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scalar_fold_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "evaluate_call_arguments_in_scope",
        "PureEvaluationFault::Fold(",
        "PureEvaluationFault::FuelExhausted",
        "catch_unwind",
        "std::ptr::eq",
        "Rc::ptr_eq",
        "serde_json::to_vec",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn shared_projection_source_keeps_native_export_queries_bounded_and_reference_compatible() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/projection_source.rs"))
            .unwrap();
    for boundary in [
        "pub trait ProjectionSourceEnvironment",
        "pub fn lower_projection_source",
        "preflight(expression, limits)",
        "preflight_names(expression)",
        ".check_pending(0, 1)",
        "preflight_with_budget(&value, 1, budget)",
        "let PureType::Result(result) = input_type",
        "valid_local_name(self.group)",
        "valid_member_name(self.name)",
        "source.construct(operation)",
        "self.construct(value, field)",
    ] {
        assert!(
            source.contains(boundary),
            "missing projection boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_projection_source")
        .unwrap()
        .1;
    assert!(
        public.find("preflight_names(expression)").unwrap()
            < public
                .find("environment.member")
                .or_else(|| public.find(".member(source.group"))
                .unwrap()
    );
    let finish = source.split_once("fn finish_with_budget").unwrap().1;
    assert!(
        finish.find("preflight_with_budget(&value").unwrap()
            < finish.find("export(&input, key)").unwrap()
    );
    assert!(
        public.find("source.finish_with_budget(").unwrap()
            < public.find(".field(result, name)").unwrap()
    );
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "evaluate_pure",
        "OperationCatalog",
        "Field: Clone",
        "type Result: Clone",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected projection coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let lowerer = reference
        .split_once("pub(super) fn lower_expression_with_functions")
        .unwrap()
        .1
        .split_once("fn lower_loop")
        .unwrap()
        .0;
    assert!(lowerer.contains("crate::projection_source::bind_field_source(arguments, span)"));
    assert!(lowerer.contains("lower_bound_projection_source("));
    assert!(lowerer.contains(".lower("));
    assert!(!lowerer.contains("Computation::Field {"));
    assert!(!lowerer.contains("Computation::Member {"));
    assert!(!reference.contains("fn named"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/projection_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "evaluate_call_arguments_in_scope",
        "lower_scalar_source",
        "catch_unwind",
        "std::ptr::eq",
        "Rc::ptr_eq",
        "serde_json::to_vec",
        "source_construction_does_not_require_clone_or_debug_for_result_observations",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn reference_field_frontend_uses_shared_consuming_stages_without_changing_public_preflight() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/projection_source.rs"))
            .unwrap();
    let production = source.split_once("#[cfg(test)]").unwrap().0;
    let staged = production
        .split_once("pub(crate) fn lower_preflighted")
        .unwrap()
        .1
        .split_once("fn finish_with_budget")
        .unwrap()
        .0;
    assert!(
        staged.find(".check_pending(0, 1)").unwrap() < staged.find("lower(self.value)").unwrap()
    );
    assert!(
        staged.find("lower(self.value)").unwrap()
            < staged.find("self.finish_with_budget(").unwrap()
    );
    let finish = production
        .split_once("fn finish_with_budget")
        .unwrap()
        .1
        .split_once("pub fn construct")
        .unwrap()
        .0;
    assert!(
        finish.find("decode(self.literal_name()?)").unwrap()
            < finish.find("preflight_with_budget(&value").unwrap()
    );
    assert!(
        finish.find("preflight_with_budget(&value").unwrap()
            < finish.find("export(&input, key)").unwrap()
    );
    assert!(finish.contains("self.construct(value, field)"));
    for coupling in [
        ".clone()",
        "evaluate_",
        "validate_in_type_scope",
        "ResultField",
        "HostOperation",
        "Field: Clone",
        "Input: Clone",
    ] {
        assert!(
            !production.contains(coupling),
            "unexpected field coupling {coupling}"
        );
    }
    let public = production
        .split_once("pub fn lower_projection_source")
        .unwrap()
        .1;
    assert!(
        public.find("preflight_names(expression)").unwrap()
            < public.find(".lower_value(source.value)").unwrap()
    );
    assert!(public.contains("source.finish_with_budget("));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let field = reference
        .split_once("if callee == \"field\"")
        .unwrap()
        .1
        .split_once("if callee == \"choose\"")
        .unwrap()
        .0;
    for boundary in [
        ".lower(",
        "lower_expression_with_functions(child, scope, visited, depth + 1, functions)",
        "ResultField::parse(name)",
        "field.result_type(*input_type)",
    ] {
        assert!(
            field.contains(boundary),
            "missing field boundary {boundary}"
        );
    }
    for duplication in [
        ".literal_name()",
        "source.construct(",
        "value.is_pure()",
        "Computation::Field {",
    ] {
        assert!(!field.contains(duplication));
    }
    assert!(
        reference
            .contains("field_stages_charge_original_child_once_without_lowering_name_metadata")
    );
    for proof in [
        "exact_original_ast_name_input_buffer_and_move_only_slots_pass_once",
        "native_child_failure_precedes_nonliteral_metadata_and_is_redacted",
        "closed_name_failure_precedes_impure_input_without_an_export_query",
        "entire_input_purity_and_one_root_budget_precede_native_export",
        "export_error_or_unwind_drops_original_input_and_key_without_retry",
    ] {
        assert!(source.contains(proof));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/field_source_reference.rs"))
            .unwrap();
    for boundary in [
        "field_metadata_errors_preserve_child_before_name_before_native_export_order",
        "lookalike_native_records_do_not_supply_foreign_field_exports",
        "field_does_not_hide_effects_in_cold_pure_input_branches",
        "helper_local_result_names_never_escape_their_original_lexical_prefix",
        "canonical_source",
        "serde_json::to_vec",
        "authorize",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn native_host_source_owns_once_preparation_admission_and_construction_without_product_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/host_source.rs")).unwrap();
    let public = source
        .split_once("pub fn lower_host_source")
        .unwrap()
        .1
        .split_once("pub fn bind_host_source_root")
        .unwrap()
        .0;
    assert!(
        public.find("preflight(expression, limits)").unwrap()
            < public.find("bind_host_source_root(expression").unwrap()
    );
    let core = source.split_once("pub fn bind_host_source_root").unwrap().1;
    assert!(core.find(".visit(0, 0, 0)").unwrap() < core.find("prepare(expression)").unwrap());
    assert!(
        core.find("prepare(expression)").unwrap()
            < core.find("admit(expression, &effect, &ty)").unwrap()
    );
    assert!(
        core.find("admit(expression, &effect, &ty)").unwrap()
            < core.find("Computation::Host").unwrap()
    );
    assert!(core.contains("Box::new(effect)"));
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        ".clone()",
        "canonical_source(",
        "validate_in_type_scope",
        "evaluate_",
        "Clone +",
        "Send +",
        "serde::",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected host source coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let lowerer = reference
        .split_once("pub(super) fn lower_expression_with_functions")
        .unwrap()
        .1
        .split_once("fn lower_loop")
        .unwrap()
        .0;
    let host = lowerer
        .split_once("crate::host_source::bind_host_source_root(")
        .unwrap()
        .1;
    for boundary in [
        "lower_effect(source)?",
        "contains_computation(effect)",
        "HostOperation::parse(callee)",
        "HostOperation::for_effect(effect)",
        "operation.map(HostOperation::result_type) == Some(*result)",
        "HostSourceError::Preparation",
        "HostSourceError::Admission",
    ] {
        assert!(
            host.contains(boundary),
            "missing native host policy {boundary}"
        );
    }
    assert!(!lowerer.contains("Computation::Host {"));
    assert!(
        lowerer.find(".finish_call(").unwrap() < lowerer.find("bind_host_source_root(").unwrap()
    );
    assert!(reference.contains(
        "native_host_source_charges_one_original_root_and_never_visits_literal_metadata"
    ));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_source.rs")).unwrap();
    for boundary in [
        "original_ast_payload_buffer_and_move_only_type_pass_once_into_one_host_root",
        "whole_cold_source_names_text_arguments_and_physical_limits_precede_native_work",
        "one_language_root_never_certifies_foreign_result_or_unbounded_opaque_graphs",
        "unrelated_numeric_and_panel_hosts_use_the_same_entry_without_product_payloads",
        "native_preparation_or_admission_unwind_drops_owned_parts_without_retry",
        "native_observations_after_handoff_are_not_saved_admission_certificates",
        "Rc::ptr_eq",
        "std::ptr::eq",
    ] {
        assert!(proof.contains(boundary));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_source_reference.rs"))
            .unwrap();
    for boundary in [
        "native_leaf_errors_keep_original_messages_priority_and_shifted_source_spans",
        "helper_native_host_unwrapping_preserves_the_direct_atomic_product_bytes",
        "literal_host_bridge_does_not_replace_the_computed_argument_call_path",
        "native_host_leaves_still_supply_closed_group_members_and_projected_result_bindings",
        "canonical_source",
        "authorize",
        "serde_json::to_vec",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn reference_member_frontend_uses_shared_bounds_and_exact_borrowed_native_group_exports() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let member = source
        .split_once("if callee == \"member\"")
        .unwrap()
        .1
        .split_once("if callee == \"field\"")
        .unwrap()
        .0;
    for boundary in [
        "lower_bound_projection_source(",
        "BoundProjectionSourceLimits",
        "PureType::Result(MemberSourceType::Bound(local))",
        "Type::Scalar(ty) => PureType::Scalar(ty)",
        "max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS",
        "PureType::Result(MemberSourceType::Selected(ty))",
        "member is not exported by this bound group",
    ] {
        assert!(
            member.contains(boundary),
            "missing reference member boundary {boundary}"
        );
    }
    for legacy in [
        "member_source(arguments",
        "scope.get(",
        ".clone()",
        "Computation::Member {",
    ] {
        assert!(
            !member.contains(legacy),
            "member frontend retained {legacy}"
        );
    }
    let adapter = source
        .split_once("fn member(\n")
        .unwrap()
        .1
        .split_once("impl From<Type>")
        .unwrap()
        .0;
    assert!(adapter.contains("MemberSourceType::Bound(local)"));
    assert!(adapter.contains("local.members().and_then"));
    assert!(adapter.contains("MemberSourceType::Selected(operation.result_type())"));
    assert!(!adapter.contains(".clone()"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/member_source_reference.rs"))
            .unwrap();
    for claim in [
        "reference_member_source_preserves_exact_native_slot_without_a_synthetic_local",
        "same_named_member_in_a_different_group_does_not_authorize_a_foreign_native_operation",
        "same_result_field_type_does_not_allow_substitution_of_another_native_operation",
        "scalar_record_missing_and_expired_helper_scopes_do_not_supply_member_exports",
        "member_header_diagnostics_keep_legacy_root_spans_and_never_lower_metadata_as_values",
        "full_width_group_last_member_and_source_metadata_limits_preserve_wire_and_authority",
        "field_child_error_precedence_is_unchanged_when_member_lowering_delegates_to_shared_core",
    ] {
        assert!(proof.contains(claim), "missing native member proof {claim}");
    }
}

#[test]
fn shared_owned_helper_hygiene_preserves_native_slots_and_reference_expansion_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_hygiene.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperHygieneLimits",
        "pub enum HelperHygieneError",
        "pub fn hygienic_helper_body",
        "parameters: &[(&str, &str)]",
        "reserved: &[&str]",
        "StructureBudget::new(limits.max_nodes, limits.max_depth)",
        ".check_pending(pending.len(), 1)",
        "names: &'names HashSet<String>",
        "ScopeFrame",
        "std::mem::replace(name, fresh.clone())",
        "self.names.contains(&name)",
        "!self.used.insert(name.clone())",
        "HelperHygieneError::Capture { group: true }",
        "check_binding(name, scope, 2, max_bindings)",
        "check_binding(item, scope, 2, max_bindings)",
        "Node::Literal { .. } | Node::Host { .. } => {}",
        "Failure/unwind drops the consumed body",
        "native name reservations are not rolled back or refunded",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper boundary {boundary}"
        );
    }
    let public = source.split_once("pub fn hygienic_helper_body").unwrap().1;
    let rename = public.find("renamer.rename(").unwrap();
    for preflight in [
        "collect_names(&body, limits, &mut names)",
        "check_scope(",
        "names.contains(alias)",
    ] {
        assert!(public.find(preflight).unwrap() < rename);
    }
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "evaluate_pure",
        "infer_pure_type",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "body.clone()",
        "scope.clone()",
        "BTreeMap",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    for boundary in [
        "crate::helper_instance_finish::finish_helper_instance(",
        "body.clone()",
        "fresh_name(self.names, self.next_name)",
        ".bindings()",
        "max_reserved_names: MAX_COMPUTATION_NODES",
        "helper cannot capture caller locals",
        "helper cannot capture a caller group",
    ] {
        assert!(
            lowerer.contains(boundary),
            "missing reference helper boundary {boundary}"
        );
    }
    assert!(
        lowerer.find("bind_helper_source_arguments(").unwrap()
            < lowerer.find("finish_helper_instance(").unwrap()
    );
    assert!(
        lowerer.find("finish_helper_instance(").unwrap()
            < lowerer.find("struct InstanceFinisher").unwrap()
    );
    assert!(!reference.contains("fn rename("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_hygiene.rs")).unwrap();
    for boundary in [
        "lower_scalar_source_with_scope",
        "parse(",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "exact_execution_fuel",
        "catch_unwind",
        "std::ptr::eq",
        "buffer.as_ptr()",
        "serde_json::to_vec",
        "call_group_and_string_forests_do_not_export_sibling_temporary_bindings",
    ] {
        assert!(proof.contains(boundary), "missing helper proof {boundary}");
    }
}

#[test]
fn shared_helper_signature_and_arguments_keep_source_borrows_native_slots_and_expansion_boundaries()
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_source.rs"))
            .unwrap();
    for boundary in [
        "pub fn helper_parameters",
        "pub struct HelperSignature",
        "pub struct HelperSourceLimits",
        "pub fn lower_helper_arguments",
        "max_parameters > MAX_FUNCTION_PARAMETERS",
        "NamedParameter::required(parameter.name.as_str(), ty)",
        "preflight::<Error>(expression, limits.source)",
        "callee != signature.name",
        "!parameter.required || !valid_local_name(parameter.name)",
        "bind_named_arguments(&names[..arguments.len()], parameters)",
        "bindings.into_iter().enumerate()",
        "let parameter = &parameters[parameter_index]",
        ".check_pending(0, arguments.len() - parameter_index)",
        "preflight_with_budget(&value, 0, &mut budget)",
        "check_argument_type(&ScalarTypeSet::only(parameter.domain), facts)",
        "pub parameter: &'signature HelperParameter<'signature>",
        "pub argument: &'source NamedArgument",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper argument boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_helper_arguments")
        .unwrap()
        .1;
    assert!(
        public.find("preflight::<Error>").unwrap()
            < public
                .find("bind_helper_source_arguments(arguments")
                .unwrap()
    );
    let preparation = source
        .split_once("pub fn bind_helper_source_arguments")
        .unwrap()
        .1;
    assert!(
        preparation.find("bind_named_arguments(").unwrap()
            < preparation.find("lower(argument)").unwrap()
    );
    assert!(
        preparation.find(".check_pending(").unwrap() < preparation.find("lower(argument)").unwrap()
    );
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "evaluate_pure",
        "infer_pure_type",
        "OperationCatalog",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "value.clone()",
        "body.clone()",
        "ScopeFrame",
        "Fuel::",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper argument coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let parameter_adapter =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_body_source.rs"))
            .unwrap();
    assert!(
        parameter_adapter
            .contains("helper_parameters(function, limits.body.template.max_parameters)")
    );
    assert!(reference.contains("crate::helper_body_source::lower_declared_helper_body("));
    assert!(!reference.contains("fn parameter_types"));
    assert!(!parameter_adapter.contains("parameter.type_name.as_str()"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(lowerer.contains("crate::helper_source::bind_helper_source_arguments("));
    assert!(!lowerer.contains("value.is_pure()"));
    assert!(
        lowerer.find("bind_helper_source_arguments(").unwrap()
            < lowerer.find("finish_helper_instance(").unwrap()
    );
    assert!(lowerer.contains("helper arguments must be pure scalars of the declared type"));
    assert!(lowerer.contains("arguments[argument_index].span"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_source.rs")).unwrap();
    for boundary in [
        "helper_parameters",
        "lower_scalar_source_with_scope",
        "hygienic_helper_body",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "before.remaining(), after.remaining()",
        "std::ptr::eq",
        "buffer.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
        "successful_dynamic_observations_do_not_replace_complete_cold_inference",
        "native_output_bounds_and_language_names_are_not_certified_by_type_observations",
    ] {
        assert!(
            proof.contains(boundary),
            "missing helper argument proof {boundary}"
        );
    }
}

#[test]
fn shared_helper_dependencies_keep_borrowed_bounded_lazy_planning_separate_from_execution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_dependencies.rs"))
            .unwrap();
    for boundary in [
        "pub const MAX_DEPENDENCY_HELPERS: usize = 31",
        "pub struct HelperDependencyLimits",
        "pub enum HelperDependencyError",
        "pub struct HelperDependencyOrder<'source>",
        "pub fn helper_dependency_order<'source>",
        "helpers: &[&'source Function]",
        "helpers: Vec<(usize, &'source Function)>",
        "dependencies: [u32; MAX_DEPENDENCY_HELPERS]",
        "StructureBudget::new(limits.max_source_nodes, limits.max_source_depth)",
        ".check_pending(pending.len(), arguments.len())",
        "helpers.len() > limits.max_helpers",
        "!valid_local_name(entry)",
        "!valid_local_name(&function.name)",
        "helpers[..declaration_index]",
        "preflight_source(function, index, limits)?",
        "helpers.sort_unstable_by",
        "function.name.as_str().cmp(callee.as_str())",
        "dependencies[index] |= 1u32 << dependency",
        "declaration_index: *declaration_index",
        "span: *span",
        "self.dependencies[index] & !self.planned == 0",
        "self.planned |= bit",
        "impl FusedIterator for HelperDependencyOrder<'_>",
        "Early abandonment does not validate the graph",
        "Discard the cursor on lowering failure",
    ] {
        assert!(
            source.contains(boundary),
            "missing dependency boundary {boundary}"
        );
    }
    let preflight = source.split_once("fn preflight_source").unwrap().1;
    assert!(
        preflight.find(".check_pending(").unwrap() < preflight.find("pending.extend(").unwrap()
    );
    let constructor = source
        .split_once("pub fn helper_dependency_order")
        .unwrap()
        .1;
    assert!(
        constructor.find("preflight_source(").unwrap()
            < constructor.find("helpers.sort_unstable_by").unwrap()
    );
    assert!(
        constructor.find("preflight_source(").unwrap()
            < constructor.find(".binary_search_by(").unwrap()
    );
    assert!(!constructor.contains("HelperDependencyError::Cycle"));
    let debug = source
        .split_once("impl fmt::Debug for HelperDependencyOrder")
        .unwrap()
        .1
        .split_once("impl<'source> Iterator")
        .unwrap()
        .0;
    assert!(debug.contains("self.planned.count_ones()"));
    assert!(!debug.contains("function.name"));
    assert!(!debug.contains("function.body"));
    for coupling in [
        ".clone()",
        "to_owned()",
        "BTreeSet",
        "crate::Type",
        "HostOperation",
        "ResultField",
        "OperationCatalog",
        "evaluate_pure",
        "infer_pure_type",
        "canonical_source",
        "Template",
        "Fuel::",
        "sqlite",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected dependency coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_dependencies;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    for boundary in [
        "crate::program_source::assemble_declared_program(",
        "max_helpers: MAX_FUNCTIONS - 1",
        "max_source_nodes: MAX_COMPUTATION_NODES",
        "max_source_depth: MAX_EFFECT_NESTING_DEPTH",
        "|function, functions|",
        "computation::is_builtin(name)",
        "crate::helper_body_source::lower_declared_helper_body(",
        "main cannot have parameters",
        "crate::helper_instance_finish::finish_helper_instance(",
        "recursive helper functions are not supported",
        "helpers cannot call main",
    ] {
        assert!(
            reference.contains(boundary),
            "missing reference dependency gate {boundary}"
        );
    }
    assert!(!reference.contains("BTreeSet"));
    assert!(!reference.contains("let mut dependencies = BTreeMap"));
    let lowerer = reference
        .split_once("assemble_declared_program(")
        .unwrap()
        .1;
    assert!(
        lowerer.find("lower_declared_helper_body(").unwrap()
            < lowerer.find("lower_expression_with_functions(").unwrap()
    );
    let assembly =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_registry.rs"))
            .unwrap();
    let program =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/program_source.rs"))
            .unwrap();
    assert!(program.contains(
        "assemble_helper_registry(declarations, limits.helpers, state, prepare, register)"
    ));
    assert!(assembly.contains("helper_dependency_order("));
    assert!(assembly.contains("for function in order"));
    assert!(
        assembly.find("function.map_err(").unwrap()
            < assembly.find("prepare(function, &mut state)").unwrap()
    );
    assert!(
        assembly
            .find("register(function, body, &mut state)")
            .unwrap()
            < assembly.find("Ok(state)").unwrap()
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_dependencies.rs"))
            .unwrap();
    for boundary in [
        "reference_order",
        "all_input_permutations_preserve_ready_order_with_chains_and_shared_dependencies",
        "dense_maximum_graph_uses_safe_bits_and_matches_the_old_order",
        "direct_mutual_and_cold_unused_cycles_are_not_reachability_pruned",
        "ready_helpers_precede_a_single_terminal_cycle_error_including_blocked_dependents",
        "all_physical_preflight_precedes_entry_call_scanning_without_mutating_source",
        "entry_call_priority_is_lexical_helper_then_rightmost_physical_child",
        "dependency_planning_is_not_signature_operand_or_native_operation_validation",
        "reference_ready_helper_lowering_error_keeps_precedence_over_later_cycle_detection",
        "std::ptr::eq",
        "helper_parameters",
        "hygienic_helper_body",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing dependency proof {boundary}"
        );
    }
}

#[test]
fn shared_owned_helper_wrappers_keep_shifted_bounds_capture_fences_and_native_slot_ownership() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_bindings.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperBinding<Node>",
        "pub struct HelperBindingLimits",
        "pub enum HelperBindingError",
        "pub fn bind_helper_arguments",
        "bindings: Vec<HelperBinding<Node<Field, Operation, HostEffect, IrResult>>>",
        "limits.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "limits.max_parameters > MAX_FUNCTION_PARAMETERS",
        "bindings.len() > limits.max_parameters",
        "!valid_local_name(&binding.name)",
        "bindings[..index]",
        "StructureBudget::new(limits.max_nodes, limits.max_depth)",
        ".visit(index, 0, 0)",
        ".check_pending(0, bindings.len() + 1)",
        ".check_pending(0, bindings.len() - index + 1)",
        "preflight_with_budget(&binding.value, index + 1, &mut budget)",
        "conflicts(node, &bindings, true)",
        "let mut pending = vec![(&body, bindings.len())]",
        "conflicts(node, &bindings, false)",
        ".check_pending(pending.len(), 1)",
        "for binding in bindings.into_iter().rev()",
        "name: binding.name",
        "value: Box::new(binding.value)",
        "body: Box::new(body)",
        "The caller also reserves the",
        "Body literals/operator/group metadata are not certified by the shape walk",
    ] {
        assert!(
            source.contains(boundary),
            "missing wrapper boundary {boundary}"
        );
    }
    let constructor = source.split_once("pub fn bind_helper_arguments").unwrap().1;
    let wrap = constructor
        .find("for binding in bindings.into_iter().rev()")
        .unwrap();
    for check in [
        "!valid_local_name(",
        "preflight_with_budget(",
        "conflicts(node, &bindings, true)",
        "conflicts(node, &bindings, false)",
        "budget\n            .visit(depth, 0, 0)",
    ] {
        assert!(constructor.find(check).unwrap() < wrap);
    }
    assert!(
        constructor.find("preflight_with_budget(").unwrap()
            < constructor
                .find("conflicts(node, &bindings, true)")
                .unwrap()
    );
    for coupling in [
        ".clone()",
        "to_owned()",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "infer_pure_type",
        "evaluate_pure",
        "FnMut",
        "fn fresh_name",
        "canonical_source",
        "ScopeFrame",
        "sqlite",
        "Fuel::",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected wrapper coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_bindings;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    let wrap = lowerer.find("finish_helper_instance(").unwrap();
    for gate in [
        "cached.revalidate(",
        "bind_helper_source_arguments(",
        "let values = lowered",
    ] {
        assert!(lowerer.find(gate).unwrap() < wrap);
    }
    assert!(lowerer.contains("max_lowered_nodes: MAX_COMPUTATION_NODES"));
    assert!(lowerer.contains("max_lowered_depth: MAX_EFFECT_NESTING_DEPTH"));
    assert!(lowerer.contains("max_parameters: MAX_FUNCTION_PARAMETERS"));
    assert!(lowerer.contains("HelperInstanceArgument"));
    assert!(!lowerer.contains("body = Computation::Bind"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_bindings.rs")).unwrap();
    for boundary in [
        "eight_parameters_fit_exactly_seventeen_nodes_and_depth_eight",
        "every_alias_is_fenced_from_all_cold_caller_operand_names",
        "cold_body_bind_loop_and_fold_declarations_cannot_shadow_parameters",
        "native_projection_boxes_buffers_and_owned_alias_strings_move_unchanged",
        "opaque_host_call_and_group_body_slots_are_not_rebuilt_or_dispatched",
        "body_frontier_is_checked_before_growth_including_cold_group_children",
        "output_shape_is_not_complete_cold_lexical_or_scalar_type_acceptance",
        "unused_argument_failure_and_native_unwind_are_not_skipped_or_retried",
        "reference_wrapper_wire_authority_and_legacy_expansion_bounds_are_preserved",
        "helper_dependency_order",
        "helper_parameters",
        "lower_helper_arguments",
        "hygienic_helper_body",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "prepare_call_in_scope",
        "std::ptr::eq",
        "buffer.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
        "remaining[0], remaining[1]",
    ] {
        assert!(proof.contains(boundary), "missing wrapper proof {boundary}");
    }
}

#[test]
fn shared_source_cost_walk_bounds_cold_trees_before_borrowed_native_observations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/source_cost.rs")).unwrap();
    for boundary in [
        "pub struct SourceCostLimits",
        "pub struct SourceCostExtra",
        "pub enum SourceCostError<Error>",
        "pub fn literal_source_extra",
        "pub fn measure_source_cost<Field, Operation, HostEffect, IrResult, Error>",
        "impl FnMut(&HostEffect) -> Result<SourceCostExtra, Error>",
        "Result<HelperExpansionCost, SourceCostError<Error>>",
        "MAX_TYPE_INFERENCE_NODES",
        "MAX_TYPE_INFERENCE_DEPTH",
        "ScalarValue::OptionalString(_) => (1, 1)",
        "value.0.len()",
        "StructureBudget::new",
        "node.children()",
        "effect.as_ref()",
        "hosts.into_iter().enumerate()",
        "host_cost(effect)",
        "checked_add(extra.depth)",
        "checked_add(extra.nodes)",
        "host_index",
        "rightmost-child-first DFS",
        "not formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing source-cost boundary {boundary}"
        );
    }
    let body = source.split_once("pub fn measure_source_cost").unwrap().1;
    let observer = body.find("host_cost(effect)").unwrap();
    assert!(
        body.find(".visit(depth, extra.nodes, extra.depth)")
            .unwrap()
            < observer
    );
    assert!(
        body.find(".check_pending(pending.len(), 1)").unwrap()
            < body.find("pending.push(").unwrap()
    );
    assert!(body.find("for child in node.children()").unwrap() < observer);
    let append = &body[observer..];
    assert!(append.find("DepthLimit").unwrap() < append.find("NodeLimit").unwrap());
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Effect::",
        "Leserpent",
        "sqlite",
        "Mutex",
        "OperationCatalog",
        "Fuel::",
        "serde::",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "infer_pure_type(",
        "evaluate_pure(",
        "body.clone()",
        "saturating_add(",
        "Default for",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected source-cost coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod source_cost;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    assert!(
        reference.contains("crate::source_cost::measure_source_cost(expression, limits, |effect|")
    );
    assert!(reference.contains("let mut effects = vec![(effect, 1)];"));
    assert!(reference.contains("budget.check_pending(effects.len(), 1)?"));
    assert!(reference.contains("prepared: admitted"));
    assert!(reference.contains("body: &template.prepared"));
    assert!(!reference.contains("pub(super) fn shape("));
    assert!(!reference.contains("literal_nodes"));
    assert!(!reference.contains("&values[index]"));
    let computation =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(computation.contains("crate::source_cost::literal_source_extra(value)"));
    let flow =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/function_flow.rs")).unwrap();
    let repeat =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computed_group.rs")).unwrap();
    assert!(flow.contains("crate::helper_join::HelperJoin::new("));
    assert!(flow.contains("functions::host_source_extra(effect, source_limits)"));
    assert!(repeat.contains("functions::source_cost(expression)"));
    assert!(!flow.contains("functions::shape("));
    assert!(!repeat.contains("functions::shape("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/source_cost.rs")).unwrap();
    for boundary in [
        "folded_literal_weights_include_optional_none_but_not_empty_list_children",
        "every_control_call_group_projection_form_uses_original_language_children",
        "complete_cold_language_tree_precedes_native_observation",
        "pending_frontier_is_bounded_before_growing_or_observing_cold_hosts",
        "native_sites_use_rightmost_first_dfs_and_relative_language_root_depth",
        "zero_iteration_loops_folds_and_recovery_still_include_all_cold_children",
        "native_depth_overflow_and_depth_before_node_priority_are_checked",
        "native_errors_move_once_without_formatter_bounds_or_payload_source_chains",
        "native_unwind_does_not_retry_rollback_effects_or_mutate_borrowed_ir",
        "move_only_native_slots_original_boxes_vectors_and_literal_buffers_are_borrowed",
        "measurement_does_not_certify_literal_bytes_lexical_types_or_native_metadata",
        "bounded_adapter_inputs_match_legacy_costs_and_independent_inclusive_limits",
        "parsed_folded_operands_compose_with_reservation_templates_hygiene_and_exact_fuel",
        "reference_helper_returns_wire_capabilities_and_cold_repeat_bounds_are_unchanged",
        "bytes.as_ptr()",
        "std::ptr::eq",
        "catch_unwind",
        "serde_json::to_vec",
        "before.remaining(), after.remaining()",
    ] {
        assert!(
            proof.contains(boundary),
            "missing source-cost proof {boundary}"
        );
    }
}

#[test]
fn shared_helper_normal_returns_admit_all_cold_copies_without_native_clone_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_returns.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperReturnLimits",
        "pub struct HelperReturnShape",
        "pub enum HelperReturnError<Error>",
        "pub struct HelperReturns<Node>",
        "MAX_TYPE_INFERENCE_NODES",
        "MAX_TYPE_INFERENCE_DEPTH",
        "budget.check_pending(pending.len(), 1)?",
        "let analysis = analyze(&value, continuation_shape)",
        "checked_mul(returns)",
        "checked_add(nodes)",
        "UnsupportedBoundary",
        "exposed_names(&value, &route)",
        "names_are_valid(&value)",
        "names_are_valid(&continuation)",
        "names_are_valid(&body)",
        "names_conflict(&continuation, &name, &reserved)",
        "pub fn return_sites",
        "pub fn connect<Error>",
        "factory(&self.continuation, current)",
        "actual != self.continuation_shape",
        "names_conflict(&body, &self.name, &self.exposed_names)",
        "Host failures are not returns",
        "Pure",
        "locals do not escape",
        "not formatting or source chains",
        "source costs",
        "No callbacks",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper return boundary {boundary}"
        );
    }
    let admit = source
        .split_once("pub fn new(")
        .unwrap()
        .1
        .split_once("pub fn return_sites")
        .unwrap()
        .0;
    let analysis = admit.find("let analysis = analyze(").unwrap();
    assert!(admit.find("preflight(&value, limits)").unwrap() < analysis);
    assert!(admit.find("preflight(&continuation, limits)").unwrap() < analysis);
    assert!(admit.find("limits.max_nodes").unwrap() < analysis);
    let copy = source.split_once("pub fn connect<Error>").unwrap().1;
    let callback = copy.find("factory(&self.continuation, current)").unwrap();
    assert!(callback < copy.find("preflight(&body, self.limits)").unwrap());
    assert!(
        copy.find("actual != self.continuation_shape").unwrap()
            < copy.find("connect(self.value, self.route").unwrap()
    );
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "Effect::",
        "Leserpent",
        "sqlite",
        "Mutex",
        "serde::",
        "body.clone()",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "Fuel::",
        "saturating_add(",
        "panic!(",
        "unreachable!(",
        ".expect(",
        ".unwrap(",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper return coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_returns;"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/function_flow.rs")).unwrap();
    assert!(adapter.contains("crate::helper_join::HelperJoin::new("));
    assert!(!adapter.contains("for (value, level) in plan.return_sites()"));
    assert!(
        adapter.find("if bytes > MAX_SOURCE_BYTES").unwrap()
            < adapter.find("plan.connect(").unwrap()
    );
    assert!(adapter.contains("body.clone()"));
    assert!(adapter.contains(".validate_structure()"));
    assert!(!adapter.contains("fn connect("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_returns.rs")).unwrap();
    for boundary in [
        "every_cold_normal_return_is_admitted_before_left_to_right_factories",
        "whole_pure_control_subtrees_are_single_returns_not_rewritten_inside",
        "unsupported_cold_recover_loop_fold_and_operand_effect_boundaries_are_rejected",
        "complete_cold_input_frontiers_are_bounded_before_return_analysis",
        "all_cold_return_copies_are_reserved_not_only_the_selected_branch",
        "pure_terminal_and_initializer_binders_do_not_capture_the_continuation",
        "cold_lexical_names_are_bounded_before_route_name_copy_or_factory_capture_hashing",
        "value_binders_cannot_capture_caller_locals_groups_or_unused_cold_names",
        "equal_node_count_but_changed_depth_is_not_a_reserved_continuation",
        "move_only_all_four_native_slots_boxes_vectors_and_buffers_move_unchanged",
        "factory_error_moves_private_error_and_drops_original_and_partial_native_values_once",
        "factory_unwind_drops_partial_owned_inputs_without_retry_or_reservation_refund",
        "physical_admission_does_not_certify_folded_source_weights_or_native_semantics",
        "unrelated_device_host_preserves_explicit_bind_typing_value_and_exact_fuel",
        "reference_adapter_preserves_canonical_wire_capabilities_and_cold_return_diagnostics",
        "bounded_normal_return_corpus_matches_legacy_desugaring_and_exact_shape",
        "native_execution_failure_never_enters_a_normal_return_or_scalar_recovery",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
        "catch_unwind",
        "arguments.as_ptr()",
    ] {
        assert!(
            proof.contains(boundary),
            "missing helper return proof {boundary}"
        );
    }
}

#[test]
fn shared_bounded_helper_join_checks_cold_costs_before_native_copy_and_whole_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_join.rs")).unwrap();
    for boundary in [
        "pub struct HelperJoinLimits",
        "pub enum HelperJoinError<Error>",
        "pub struct HelperJoin<Node>",
        "pub fn new<Error>",
        "pub fn connect<Error>",
        "pub const fn source_cost",
        "HelperReturns::new(name, value, continuation, limits.output)",
        "language_join_cost(&returns, limits.source)?",
        "measure_observed(returns.value(), limits.source, &mut observe)?",
        "observations.insert(node, extra)",
        "std::ptr::from_ref(node)",
        "Zero-sized native slots are distinct occurrences",
        "checked_mul(plan.shape().returns)",
        "cost.depth.max(continuation.depth)",
        "HelperJoinError::MissingObservation",
        "actual != self.language_cost",
        "admit(&expression, self.source_cost)",
        "not a global reservation, type or authority token",
        "Equal physical",
        "not formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper join boundary {boundary}"
        );
    }
    let build = source
        .split_once("pub fn new<Error>")
        .unwrap()
        .1
        .split_once("pub fn connect<Error>")
        .unwrap()
        .0;
    let gates = [
        "limits.source.max_nodes",
        "HelperReturns::new(",
        "language_join_cost(",
        "let mut observations",
        "measure_observed(returns.value()",
        "measure_observed(returns.continuation()",
        "let source_cost = joined_cost(",
        "Ok(Self",
    ];
    for pair in gates.windows(2) {
        assert!(build.find(pair[0]).unwrap() < build.find(pair[1]).unwrap());
    }
    let connect = source.split_once("pub fn connect<Error>").unwrap().1;
    assert!(
        connect.find(".connect(factory)").unwrap()
            < connect
                .find("language_cost(&expression, self.limits.source)")
                .unwrap()
    );
    assert!(
        connect.find("actual != self.language_cost").unwrap()
            < connect
                .find("admit(&expression, self.source_cost)")
                .unwrap()
    );
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "Effect::",
        "Leserpent",
        "sqlite",
        "Mutex",
        "serde::",
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "Fuel::",
        "saturating_add(",
        "panic!(",
        "unreachable!(",
        ".expect(",
        ".unwrap(",
        "Default for",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper join coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_join;"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/function_flow.rs")).unwrap();
    assert!(adapter.contains("crate::helper_join::HelperJoin::new("));
    assert!(adapter.contains("functions::host_source_extra(effect, source_limits)"));
    assert!(!adapter.contains("functions::source_cost("));
    assert!(
        adapter.find("if bytes > MAX_SOURCE_BYTES").unwrap()
            < adapter.find("plan.connect(").unwrap()
    );
    assert!(adapter.contains("|expression, _| expression.validate_structure()"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_join.rs")).unwrap();
    for boundary in [
        "exact_leaf_cost_is_admitted_before_once_whole_output_admission",
        "zero_invalid_physical_name_capture_and_boundary_limits_precede_native_queries",
        "both_cold_folded_inputs_are_checked_before_observing_the_first_native_value",
        "combined_cold_continuation_weights_are_bounded_before_native_observation",
        "original_hosts_are_observed_once_and_copies_keep_left_to_right_order",
        "native_extra_overflow_and_shifted_return_depth_are_checked_before_copy",
        "zero_sized_native_slots_do_not_merge_original_occurrence_costs",
        "same_physical_copy_with_changed_folded_node_weights_is_rejected_before_admission",
        "same_physical_copy_with_changed_folded_depth_is_rejected_before_admission",
        "equal_cost_does_not_certify_cold_native_semantics_or_live_authority",
        "move_only_all_four_slots_original_boxes_vectors_and_buffers_move_unchanged",
        "native_observation_error_keeps_private_payload_and_drops_owned_inputs_once",
        "native_observation_unwind_drops_inputs_without_retry_or_materialization",
        "copy_error_and_unwind_release_partial_output_without_admission_or_refund",
        "admission_error_and_unwind_never_publish_or_retry_the_complete_output",
        "unrelated_scalar_host_keeps_full_typing_result_fuel_and_caller_prefix",
        "bounded_return_corpus_costs_match_complete_post_copy_observation",
        "std::ptr::eq",
        "bytes.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing helper join proof {boundary}"
        );
    }
}

#[test]
fn shared_helper_expansion_reservation_preserves_source_costs_without_execution_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_expansion.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperExpansionCost",
        "pub struct HelperExpansionLimits",
        "pub enum HelperExpansionError<Error>",
        "pub fn reserve_helper_expansion<Error>",
        "used_nodes: &mut usize",
        "caller_depth: usize",
        "parameter_count: usize",
        "impl FnMut(usize) -> Result<usize, Error>",
        "MAX_TYPE_INFERENCE_NODES",
        "MAX_TYPE_INFERENCE_DEPTH",
        "MAX_FUNCTION_PARAMETERS",
        "checked_add(body.nodes)",
        "checked_add(parameter_count)",
        "checked_add(body.depth)",
        "checked_add(depth)",
        "BodyDepthLimit",
        "ArgumentDepthLimit",
        "parameter_index",
        "*used_nodes = total;",
        "not HelperTemplateShape",
        "already been",
        "A later downstream failure must not undo",
        "trusted adapter observations",
        "never formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing expansion boundary {boundary}"
        );
    }
    let body = source
        .split_once("pub fn reserve_helper_expansion")
        .unwrap()
        .1;
    let observer = body.find("argument_depth(parameter_index)").unwrap();
    for gate in [
        "InvalidLimits",
        "ParameterLimit",
        "EmptyBody",
        "NodeLimit",
        "BodyDepthLimit",
    ] {
        assert!(body.find(gate).unwrap() < observer);
    }
    assert!(body.find("NodeLimit").unwrap() < body.find("BodyDepthLimit").unwrap());
    assert!(observer < body.find("*used_nodes = total;").unwrap());
    for coupling in [
        "use crate::ir",
        "Computation<",
        "Effect::",
        "HostOperation",
        "ResultField",
        "Leserpent",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "Default for",
        "Clone for",
        "OperationCatalog",
        "infer_pure_type(",
        "evaluate_pure(",
        "body.clone()",
        "saturating_add(",
        "Vec::",
        "HashMap",
        "BTreeMap",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected expansion coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_expansion;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    assert!(
        reference.contains("crate::source_cost::measure_source_cost(expression, limits, |effect|")
    );
    assert!(reference.contains("let mut effects = vec![(effect, 1)];"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    for boundary in [
        "crate::helper_instance_finish::finish_helper_instance(",
        "body: &template.prepared",
        "caller_depth: depth",
        "max_source_nodes: MAX_COMPUTATION_NODES",
        "max_source_depth: MAX_EFFECT_NESTING_DEPTH",
        "max_parameters: MAX_FUNCTION_PARAMETERS",
        ".get(index)",
        "source_cost(value)",
        "LSH1405",
    ] {
        assert!(
            lowerer.contains(boundary),
            "missing expansion adapter {boundary}"
        );
    }
    assert!(!lowerer.contains("*visited += template.nodes"));
    assert!(!lowerer.contains(".saturating_add(template.nodes)"));
    assert!(
        lowerer.find("bind_helper_source_arguments(").unwrap()
            < lowerer.find("finish_helper_instance(").unwrap()
    );
    let proof = std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_expansion.rs"))
        .unwrap();
    for boundary in [
        "append_body_and_wrappers_without_recharging_call_or_operands",
        "node_rejection_precedes_shifted_depth_and_observation",
        "body_depth_overflow_rejects_before_native_argument_work",
        "observers_run_once_in_declaration_order_and_stop_at_first_bad_depth",
        "native_errors_move_once_and_never_enter_formatting_or_source_chains",
        "native_unwind_preserves_counter_without_retry_or_side_effect_rollback",
        "committed_reservations_survive_later_factory_failure_and_unwind",
        "committed_reservation_is_not_refunded_by_wrapper_rejection",
        "move_only_native_operands_are_borrowed_not_cloned_or_owned_by_reservation",
        "folded_and_opaque_source_weights_are_not_physical_template_shape",
        "supplied_metadata_cannot_certify_cold_types_or_actual_output_shape",
        "valid_adapter_inputs_match_legacy_short_circuit_arithmetic_and_observer_order",
        "parsed_declarations_to_reserved_templates_hygiene_and_exact_interpreter_fuel",
        "reference_wire_fresh_names_authority_and_expansion_diagnostics_are_unchanged",
        "std::ptr::eq",
        "bytes.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
        "before.remaining(), after.remaining()",
    ] {
        assert!(
            proof.contains(boundary),
            "missing expansion proof {boundary}"
        );
    }
}

#[test]
fn shared_closed_group_exports_preflight_cold_routes_before_explicit_native_identity_hooks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/group_exports.rs"))
            .unwrap();
    for boundary in [
        "pub struct GroupExportLimits",
        "pub struct GroupExport<'name, Operation>",
        "pub struct GroupExports<'name, Operation>",
        "pub enum GroupExportPhase",
        "pub enum GroupExportError<Error>",
        "MAX_GROUP_EXPORT_MEMBERS: usize = 64",
        "arguments.len() <= MAX_CALL_ARGUMENTS",
        "valid_argument_name(&argument.name)",
        "limits.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "limits.max_groups > MAX_TYPE_INFERENCE_NODES",
        "limits.max_members > MAX_GROUP_EXPORT_MEMBERS",
        "check_pending(pending.len(), 1)",
        "let mut known: Option<&Node",
        "!atomic_candidate(&branch.value)",
        "left.name != right.name",
        "signature = Some(current)",
        "name: &member.name",
        "branch(member)",
        "same(&expected.operation, &current.operation)",
        "No native Clone/Debug/serde/Send/PartialEq bound",
        "not full source lowering, flow typing",
        "Native errors are matchable but redacted",
    ] {
        assert!(
            source.contains(boundary),
            "missing group export boundary {boundary}"
        );
    }
    let observe = source
        .split_once("pub fn observe_group_exports<")
        .unwrap()
        .1;
    let gates = [
        "return Err(GroupExportError::InvalidLimits)",
        "physical(expression, limits)?",
        "let leaves = routes(expression, limits)?",
        "for (group_index, node)",
        "host(effect)",
        "if !member_shape(",
        "if let Some(expected) = &signature",
        "same(&expected.operation, &current.operation)",
        "signature = Some(current)",
    ];
    for pair in gates.windows(2) {
        assert!(observe.find(pair[0]).unwrap() < observe.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        "Operation: Clone",
        "Export: Clone",
        "Export: PartialEq",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "canonical_source",
        "catch_unwind",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected group export coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let adapter = reference
        .split_once("fn group_signature(")
        .unwrap()
        .1
        .split_once("impl crate::computation_profile::ComputationProfile")
        .unwrap()
        .0;
    assert!(
        adapter
            .find("expression.validate_structure().ok()?")
            .unwrap()
            < adapter
                .find("crate::group_exports::observe_group_exports(")
                .unwrap()
    );
    assert!(adapter.contains("branch.value.prepared_atomic_operation().ok_or(())"));
    assert!(adapter.contains("crate::pure_reference::native_group_exports(effect).ok_or(())"));
    let native =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    let native = native
        .split_once("pub(crate) fn native_group_exports")
        .unwrap()
        .1
        .split_once("impl<'schema>")
        .unwrap()
        .0;
    assert!(native.contains("HostOperation::for_effect(&branch.effect).ok_or(())?"));
    assert!(adapter.contains("|left, right| Ok(left == right)"));
    assert!(!adapter.contains("let mut pending"));
    assert!(reference.contains("let group_member_count = members.as_ref().map(Vec::len);"));
    assert!(reference.contains("group_member_count.is_none_or(|count|"));
    assert_eq!(reference.matches("group_members(&value,").count(), 1);
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod group_exports;"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_exports.rs")).unwrap();
    for boundary in [
        "all_known_cold_group_kinds_names_and_order_match_before_native_queries",
        "cold_call_argument_names_and_count_are_bounded_before_native_operation_queries",
        "zero_loop_and_fold_do_not_hide_effectful_preparation",
        "opaque_observers_borrow_original_graphs_once_and_join_rightmost_first",
        "native_operation_schema_identity_is_explicit_not_same_spelling_or_result_tag",
        "original_branch_return_declaration_is_available_for_native_corroboration",
        "move_only_observations_keep_buffers_without_clone_debug_serde_send_or_equality",
        "all_four_move_only_ir_slots_remain_original_borrowed_objects",
        "branch_error_releases_partial_owned_observations_without_retry_or_consuming_ir",
        "native_unwind_stops_later_hooks_and_drops_observations_without_partial_output",
        "distinct_embedded_host_formats_can_preserve_original_native_schema_identity",
        "parsed_native_source_calls_feed_closed_exports_and_prepare_values_with_exact_schema_and_fuel",
        "parsed_reference_helper_groups_preserve_canonical_wire_and_all_cold_authority",
        "reference_cold_group_reordering_result_forgery_and_nonflat_helpers_are_rejected",
    ] {
        assert!(
            proof.contains(boundary),
            "missing group export proof {boundary}"
        );
    }
}

#[test]
fn shared_selected_helper_instances_reserve_before_copy_and_admit_complete_hygienic_output() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_instance.rs"))
            .unwrap();
    let finish = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/helper_instance_finish.rs"),
    )
    .unwrap();
    for contract in [
        "pub fn lower_helper_instance",
        "pub trait HelperInstanceAdapter",
        "pub struct SelectedHelper",
        "preflight_template(template.parameters(), template.body(), template_limits)",
        "lower_helper_arguments(",
        "collect_names(&argument.value, hygiene, &mut reserved)",
        "reserve_helper_expansion(",
        "hygienic_helper_body(body, &pairs, &reserved, hygiene",
        "bind_helper_arguments(",
        "Ok((value, template.result_type()))",
        "A committed reservation is never refunded",
        "Matching physical shape alone is not semantic identity",
    ] {
        assert!(
            source.contains(contract) || finish.contains(contract),
            "missing shared instance contract {contract}"
        );
    }
    let public = source.split_once("pub fn lower_helper_instance").unwrap().1;
    for forbidden in [
        "Clone",
        "PartialEq",
        "Serialize",
        "Deserialize",
        "Send",
        "unsafe",
        "serde_json",
        "sqlite",
        "leserpent",
        "crate::lower(",
        "catch_unwind",
        "rollback",
    ] {
        assert!(
            !public.contains(forbidden),
            "unexpected instance dependency {forbidden}"
        );
    }
    let stages = [
        "preflight_instance(",
        "lower_helper_arguments(",
        "finish_preflighted_instance(",
    ];
    let mut previous = 0;
    for stage in stages {
        let position = public.find(stage).unwrap();
        assert!(position >= previous, "out-of-order helper stage {stage}");
        previous = position;
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_instance;"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_instance.rs")).unwrap();
    for contract in [
        "parsed_reordered_arguments_run_once_in_declaration_order_with_original_result_and_hygienic_scope",
        "all_six_scalar_results_support_zero_parameter_instances_without_native_metadata_copy",
        "exact_combined_output_and_source_reservations_precede_factory_work",
        "caller_unused_names_and_cold_operand_names_cannot_be_captured_by_parameter_aliases",
        "successful_reservations_survive_later_errors_and_unwind_without_retries_or_partial_publication",
        "factory_shape_changes_and_same_shape_cold_type_corruption_never_publish_an_instance",
        "native_operand_and_materialized_effect_buffers_move_once_while_cached_body_stays_owned",
        "cached_body_under_current_limits_is_checked_before_any_native_argument_work",
        "duplicate_parameter_aliases_and_bad_local_names_stop_before_final_admission",
        "unrelated_host_requires_exact_declared_projection_instead_of_an_unknown_source_fallback",
        "field.buffer.as_ptr(), adapter.argument_buffers[0]",
        "PureValue::Scalar(ScalarValue::Integer(13))",
    ] {
        assert!(
            proof.contains(contract),
            "missing instance proof {contract}"
        );
    }
    assert!(!proof.contains("leselang_hir::lower("));
}

#[test]
fn staged_helper_completion_is_shared_and_the_reference_keeps_one_recursive_counter() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/helper_instance_finish.rs"),
    )
    .unwrap();
    for contract in [
        "pub struct HelperInstanceArgument<Node>",
        "pub trait HelperInstanceFinisher",
        "pub fn finish_helper_instance",
        "fn preflight_instance",
        "fn finish_preflighted_instance",
        "admit_argument",
        "arguments.len() != parameters.len()",
        "argument.scalar_type",
        "ScalarTypeSet::only(*expected)",
        "selected.body.source_cost()",
        "phase: HelperInstancePhase::Argument { index }",
        "used_source_nodes",
        "reserved_names",
        "original borrowed result",
    ] {
        assert!(
            source.contains(contract),
            "missing staged helper contract {contract}"
        );
    }
    let completion = source
        .split_once("pub(crate) fn finish_preflighted_instance")
        .unwrap()
        .1;
    let gates = [
        "arguments.len() != parameters.len()",
        "let hygiene",
        "physical(&argument.value",
        "check_argument_type(",
        ".admit_argument(",
        "reserve_helper_expansion(",
        ".materialize(",
        "let mut aliases",
        "hygienic_helper_body(",
        "bind_helper_arguments(",
        ".admit(",
    ];
    for pair in gates.windows(2) {
        assert!(completion.find(pair[0]).unwrap() < completion.find(pair[1]).unwrap());
    }
    for forbidden in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "serde::",
        "sqlite",
        "Mutex",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "panic!(",
        "catch_unwind",
        "unsafe",
        "Fuel::",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected completion dependency {forbidden}"
        );
    }
    let original =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_instance.rs"))
            .unwrap();
    assert!(original.contains("finish_preflighted_instance("));
    assert!(!original.contains("reserve_helper_expansion("));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let call = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(call.contains("crate::helper_instance_finish::finish_helper_instance("));
    assert!(!call.contains("prepared: admitted"));
    assert!(reference.contains("prepared: crate::helper_body::HelperBody<Computation, Type>"));
    for obsolete in [
        "nodes: cost.nodes",
        "depth: cost.depth",
        "reserve_helper_expansion(",
        "hygienic_helper_body(",
        "bind_helper_arguments(",
        "let mut copied_counter",
    ] {
        assert!(
            !reference.contains(obsolete),
            "obsolete helper adapter stage {obsolete}"
        );
    }
    assert!(
        call.find("cached.revalidate(").unwrap()
            < call.find("bind_helper_source_arguments(").unwrap()
    );
    assert!(
        call.find("bind_helper_source_arguments(").unwrap()
            < call.find("finish_helper_instance(").unwrap()
    );
    assert!(call.contains("visited,\n"));
    assert!(call.contains("computation::validate_in_type_scope(value, self.scope)"));
    let computation =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(computation.contains("fn validate_in_type_scope("));
    assert!(computation.contains("fn validate_scoped("));
    assert!(computation.contains("scope_len > max_bindings"));
    assert!(computation.contains("group.members.len() > max_group_members"));
    assert!(computation.contains("&mut visited, root_depth"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_instance_finish.rs"))
            .unwrap();
    for evidence in [
        "already_compiled_operands_share_the_original_counter_and_once_finish_order",
        "cached_and_combined_limits_precede_operand_native_hooks_or_counter_changes",
        "argument_count_and_all_cold_scalar_facts_precede_every_native_hook",
        "a_complete_cold_argument_forest_is_bounded_before_any_native_type_observation",
        "forged_unused_native_argument_type_is_rejected_before_depth_copy_or_commit",
        "original_move_only_operand_buffer_and_cached_result_observation_are_retained",
        "native_argument_error_and_unwind_release_operands_without_copy_or_commit",
        "all_native_operand_types_are_admitted_before_any_depth_query_or_copy",
        "source_depth_overflow_and_insufficient_budget_leave_the_same_counter_unchanged",
        "later_copy_alias_and_admission_error_or_unwind_never_refund_committed_expansion",
        "zero_parameter_completion_still_copies_and_admits_once_without_operand_hooks",
        "reference_nested_same_helper_operands_groups_and_unused_types_keep_legacy_contracts",
        "reference_deep_fold_prefix_uses_binding_quota_not_expression_root_depth",
        "reference_full_width_caller_group_keeps_exact_last_member_signature",
    ] {
        assert!(
            proof.contains(evidence),
            "missing staged helper proof {evidence}"
        );
    }
}

#[test]
fn shared_helper_body_source_owns_closed_signature_and_original_counter_before_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_body_source.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperBodySourceLimits",
        "pub struct HelperBodyObservation<Node, ResultType>",
        "pub enum HelperBodySourceError<Lowering, Validation, Cost>",
        "pub fn lower_helper_body<",
        "pub fn lower_declared_helper_body<",
        "lower: impl FnOnce(",
        "&[HelperParameter<'source>]",
        "used_source_nodes: &mut usize",
        "valid_limits(source_limits(limits))",
        "limits.body.template.max_bindings > MAX_TYPE_INFERENCE_BINDINGS",
        "limits.body.template.max_parameters > MAX_FUNCTION_PARAMETERS",
        "limits.body.source_cost.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.body.source_cost.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "helper_parameters(function, limits.body.template.max_parameters)",
        "parameters.len() > limits.body.template.max_bindings",
        "used_source_nodes > limits.max_source_nodes",
        "*used_source_nodes > limits.max_source_nodes",
        "SourceCounterLimit",
        "SourceCallError::Shape { span, error }",
        "parameter.name.to_owned(), parameter.domain",
        ".prepare(limits.body, validate, host_cost)",
        "Native accounting is trusted; counter bounds do not prove fidelity",
        "No copied meter, precharge, rollback or retry",
        "not a generic recursive compiler",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper body source boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_helper_body<")
        .unwrap()
        .1
        .split_once("pub fn lower_declared_helper_body<")
        .unwrap()
        .0;
    assert!(
        public
            .find("prepare_header(function, limits, *used_source_nodes)")
            .unwrap()
            < public.find("preflight::<Lowering>").unwrap()
    );
    assert!(public.find("preflight::<Lowering>").unwrap() < public.find("finish(").unwrap());
    let completion = source.split_once("fn finish<").unwrap().1;
    let gates = [
        "lower(source.function, &source.parameters, used_source_nodes)",
        "*used_source_nodes > limits.max_source_nodes",
        "LoweredHelperBody {",
        ".prepare(limits.body, validate, host_cost)",
    ];
    for pair in gates.windows(2) {
        assert!(completion.find(pair[0]).unwrap() < completion.find(pair[1]).unwrap());
    }
    let observation = source
        .split_once("pub struct HelperBodyObservation<Node, ResultType>")
        .unwrap()
        .1
        .split_once("pub enum HelperBodySourceError")
        .unwrap()
        .0;
    assert!(!observation.contains("parameters"));
    for coupling in [
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "ResultType: Clone",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "serde::",
        "sqlite",
        "Fuel::",
        "catch_unwind",
        "RefCell",
        "Mutex",
        "let mut visited",
        "*used_source_nodes =",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper source coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_body_source;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let body = reference
        .split_once("assemble_declared_program(")
        .unwrap()
        .1
        .split_once("|main, functions|")
        .unwrap()
        .0;
    for boundary in [
        "lower_declared_helper_body(",
        "|function, parameters, visited|",
        "LocalType::from(Type::Scalar(parameter.domain))",
        "&function.body",
        "visited,",
        "let checked = body.validate_in_scope(",
        "if checked != *result_type",
        "host_source_extra(effect, source_limits)",
        "HelperBodySourceError::Lowering(error)",
        "prepared: admitted",
    ] {
        assert!(
            body.contains(boundary),
            "missing product body delegation {boundary}"
        );
    }
    assert!(!body.contains("crate::helper_body::LoweredHelperBody"));
    assert!(!body.contains("parameter_types(function)"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_body_source.rs"))
            .unwrap();
    for evidence in [
        "six_closed_scalar_tokens_and_original_declaration_order_own_the_signature",
        "every_source_output_prefix_and_cost_ceiling_precedes_native_lowering",
        "complete_cold_source_names_text_arity_nodes_and_depth_precede_hooks",
        "lowerer_cannot_replace_signature_or_capture_undeclared_parameters",
        "original_counter_is_borrowed_once_and_successful_overcharge_drops_output_without_refund",
        "once_lowering_validation_and_cost_keep_original_native_buffers_and_owned_policy",
        "validation_cost_errors_and_unwind_release_outputs_without_partial_body_or_counter_refund",
        "lowering_error_and_unwind_preserve_counter_and_never_enter_admission",
        "parsed_scalar_source_preparation_instance_and_value_keep_exact_counter_fuel_and_scope",
        "reference_body_pipeline_keeps_wire_cold_authority_nested_helpers_and_error_priority",
        "returned_scalar_category_still_requires_native_metadata_corroboration",
        "assert_eq!(body_counter, 13)",
        "assert_eq!(caller_counter, 6)",
        "assert_eq!(fuel.remaining(), 95)",
    ] {
        assert!(
            proof.contains(evidence),
            "missing helper body source proof {evidence}"
        );
    }
}

#[test]
fn shared_lowered_helper_body_admission_corroborates_cold_types_before_native_cost_and_storage() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_body.rs")).unwrap();
    for boundary in [
        "pub struct LoweredHelperBody<Node, ResultType>",
        "pub struct HelperBody<Node, ResultType>",
        "pub struct HelperBodyLimits",
        "pub enum HelperBodyError<Validation, Cost>",
        "pub fn into_parts(self)",
        "validate: impl FnOnce(",
        "host_cost: impl FnMut(&HostEffect)",
        "limits.source_cost.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.source_cost.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "PureResultMismatch",
        "struct ClosedScalarParameters;",
        "type Result = ();",
        "PureType::Scalar(*ty)",
        "max_bindings: limits.template.max_bindings",
        "template.result_type()",
        "self.scalar_result",
        "No inferred type is trusted solely",
        "No native Clone/Debug/serde/Send bound",
        "Cost limits/failure do not undo earlier validation work",
        "Native errors remain matchable, not formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing body admission boundary {boundary}"
        );
    }
    let prepare = source
        .split_once("pub fn prepare<Validation, Cost>")
        .unwrap()
        .1;
    let gates = [
        "limits.source_cost.max_nodes >",
        "let template = HelperTemplate::new(",
        "if template.body().is_pure()",
        "let inferred = infer_pure_type(",
        "if inferred != declared",
        "validate(",
        "let source_cost = measure_source_cost(",
        "Ok(HelperBody {",
    ];
    for pair in gates.windows(2) {
        assert!(prepare.find(pair[0]).unwrap() < prepare.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "ResultType: Clone",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "canonical_source",
        "catch_unwind",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected body admission coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_body;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let gates = [
        "crate::helper_body_source::lower_declared_helper_body(",
        "computation::lower_expression_with_functions(",
        "crate::helper_body_source::HelperBodyObservation",
        "let checked = body.validate_in_scope(",
        "if checked != *result_type",
        ".insert(function.name.clone(), Template",
        "prepared: admitted",
    ];
    for pair in gates.windows(2) {
        assert!(reference.find(pair[0]).unwrap() < reference.find(pair[1]).unwrap());
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_body.rs")).unwrap();
    for boundary in [
        "every_scalar_domain_is_independently_inferred_before_native_validation",
        "forged_pure_return_observation_is_denied_before_any_native_callback",
        "zero_loop_and_fold_next_bodies_are_still_typed",
        "cold_physical_and_lexical_preflight_precedes_purity_and_callbacks",
        "pure_native_projection_cannot_invent_a_result_from_scalar_parameters",
        "native_validation_corroborates_result_metadata_after_shared_scalar_inference",
        "all_move_only_native_slots_and_owned_buffers_are_handed_off_unchanged",
        "complete_cold_host_validation_precedes_rightmost_first_cost_observations",
        "validation_error_prevents_cost_and_partial_registry_entry_and_drops_inputs_once",
        "cost_error_retains_private_error_without_retry_refund_or_partial_output",
        "validation_and_cost_unwind_drop_owned_inputs_once_without_catching_or_retrying",
        "parsed_scalar_helper_admission_materialization_binding_and_returns_keep_value_and_exact_fuel",
        "reference_helpers_preserve_canonical_wire_authority_and_unused_cold_rejection",
    ] {
        assert!(
            proof.contains(boundary),
            "missing body admission proof {boundary}"
        );
    }
}

#[test]
fn shared_owned_helper_templates_preflight_once_factories_without_native_copy_or_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_templates.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperTemplate<Node, ResultType>",
        "parameters: Vec<(String, ScalarType)>",
        "body: Node,",
        "result_type: ResultType,",
        "pub struct HelperTemplateLimits",
        "pub struct HelperTemplateShape",
        "pub enum HelperTemplateError<Error>",
        "Body(HelperHygieneError<Infallible>)",
        "limits.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS",
        "limits.max_parameters > MAX_FUNCTION_PARAMETERS",
        "parameters.len() > limits.max_parameters",
        "parameters.len() > limits.max_bindings",
        "!valid_local_name(name) || !names.insert(name.to_owned())",
        "pub fn parameters(&self) -> &[(String, ScalarType)]",
        "pub fn body(&self) -> &Node",
        "pub fn result_type(&self) -> &ResultType",
        "pub fn into_parts(self) -> (Vec<(String, ScalarType)>, Node, ResultType)",
        "(self.parameters, self.body, self.result_type)",
        "factory: impl FnOnce(",
        "No operands, aliases, result receipt",
        "needs fresh caller-owned admission and expansion accounting",
    ] {
        assert!(
            source.contains(boundary),
            "missing template boundary {boundary}"
        );
    }
    let preflight = source.split_once("fn preflight<").unwrap().1;
    assert!(preflight.find("collect_names(").unwrap() < preflight.find("check_scope(").unwrap());
    let materialize = source.split_once("pub fn materialize<Error>").unwrap().1;
    let gates = [
        "preflight(&self.parameters, &self.body, limits)",
        "factory(&self.body).map_err(HelperTemplateError::Factory)",
        "preflight(&self.parameters, &body, limits)",
        "if expected != actual",
        "Ok(body)",
    ];
    for pair in gates.windows(2) {
        assert!(materialize.find(pair[0]).unwrap() < materialize.find(pair[1]).unwrap());
    }
    let debug = source
        .split_once("fmt::Debug for HelperTemplate<Node, ResultType>")
        .unwrap()
        .1
        .split_once("impl<Node, ResultType> HelperTemplate<Node, ResultType>")
        .unwrap()
        .0;
    assert!(debug.contains("&self.parameters.len()"));
    assert!(debug.contains("&self.shape"));
    assert!(!debug.contains("&self.body"));
    assert!(!debug.contains("&self.result_type"));
    for coupling in [
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "infer_pure_type",
        "evaluate_pure",
        "FnMut",
        "canonical_source",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected template coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_templates;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    assert!(reference.contains("prepared: crate::helper_body::HelperBody<Computation, Type>"));
    let checked = reference.find("body.validate_in_scope(").unwrap();
    let stored = reference.find("prepared: admitted").unwrap();
    assert!(
        reference
            .find("crate::helper_body_source::HelperBodyObservation")
            .unwrap()
            < checked
    );
    assert!(checked < reference.find("if checked != *result_type").unwrap());
    assert!(checked < stored);
    assert!(
        reference
            .find(".insert(function.name.clone(), Template")
            .unwrap()
            < stored
    );
    assert!(
        reference.contains("crate::source_cost::measure_source_cost(expression, limits, |effect|")
    );
    assert!(reference.contains("let mut effects = vec![(effect, 1)];"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(lowerer.contains("cached.parameters().to_vec()"));
    assert!(lowerer.contains("Ok(body.clone())"));
    assert!(lowerer.contains("Ok((body, *result_type))"));
    let materialize = lowerer.find("finish_helper_instance(").unwrap();
    assert!(lowerer.find("bind_helper_source_arguments(").unwrap() < materialize);
    assert!(
        materialize
            < lowerer
                .find("fresh_name(self.names, self.next_name)")
                .unwrap()
    );
    assert!(materialize < lowerer.find("fn admit(&mut self").unwrap());
    assert!(!lowerer.contains("crate::helper_bindings::bind_helper_arguments("));
    let proof = std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_templates.rs"))
        .unwrap();
    for boundary in [
        "owned_signature_body_and_return_metadata_move_unchanged_and_can_be_consumed",
        "zero_loop_and_fold_paths_still_have_complete_lexical_preflight",
        "physical_frontier_and_depth_bounds_precede_lexical_scope_walks",
        "debug_and_factory_error_never_dump_private_names_literals_or_native_metadata",
        "narrower_call_limits_reject_before_the_factory_even_for_cold_bindings",
        "factory_receives_exact_original_borrow_once_and_can_move_gui_local_slots",
        "returned_free_locals_and_parameter_shadowing_fail_without_partial_output",
        "smaller_or_differently_nested_output_cannot_change_reserved_physical_shape",
        "expanded_factory_output_is_bounded_and_its_native_slots_drop_once",
        "factory_error_and_unwind_preserve_cache_without_retry_or_native_side_effect_rollback",
        "cached_opaque_host_call_group_and_result_vectors_are_owned_without_rebuilding",
        "equal_shape_does_not_certify_literal_operator_or_declared_return_type_identity",
        "repeated_explicit_materialization_matches_original_value_and_exact_interpreter_fuel",
        "parsed_helper_pipeline_owns_materializes_and_prepares_an_unrelated_native_call",
        "body_parameter_types_and_source_expansion_weights_are_not_template_shape_certificates",
        "reference_repeated_nested_templates_keep_wire_diagnostics_and_cold_authority",
        "std::ptr::eq",
        "bytes.as_ptr()",
        "catch_unwind",
        "prepare_call_in_scope",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing template proof {boundary}"
        );
    }
}

#[test]
fn shared_owned_helper_registry_keeps_lazy_preparation_and_complete_native_state_handoff() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_registry.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperRegistryLimits",
        "pub enum HelperRegistryError<Lowering, Registration>",
        "pub type HelperRegistryResult<State, Lowering, Registration>",
        "pub fn assemble_helper_registry<",
        "declarations: &HelperDeclarations<'source>",
        "mut state: State",
        "mut prepare: impl FnMut(",
        "mut register: impl FnMut(",
        "&'source Function",
        "&mut State",
        "declarations.helpers()",
        "&declarations.entry().name",
        "for function in order",
        "helper_parameters(function, limits.template.max_parameters)",
        "parameters.len() > limits.template.max_bindings",
        "parameters.len() != template.parameters().len()",
        "declared.name != name || declared.domain != *ty",
        "preflight(template.parameters(), template.body(), limits.template)",
        "not transactional rollback",
        "not complete generic recursive source compilation",
        "callbacks must not dispatch or publish execution",
        "prepare must",
        "copied",
        "instances and dispatch need fresh validation",
    ] {
        assert!(
            source.contains(boundary),
            "missing registry boundary {boundary}"
        );
    }
    let assembly = source
        .split_once("pub fn assemble_helper_registry<")
        .unwrap()
        .1;
    let order = assembly
        .find("let order = helper_dependency_order(")
        .unwrap();
    for ceiling in [
        "limits.dependencies.max_helpers >",
        "limits.dependencies.max_source_nodes >",
        "limits.dependencies.max_source_depth >",
        "limits.template.max_nodes >",
        "limits.template.max_depth >",
        "limits.template.max_bindings >",
        "limits.template.max_parameters >",
        "limits.source_cost.max_nodes >",
        "limits.source_cost.max_depth >",
    ] {
        assert!(assembly.find(ceiling).unwrap() < order);
    }
    let gates = [
        "function.map_err(HelperRegistryError::Dependency)",
        "helper_parameters(function",
        "parameters.len() > limits.template.max_bindings",
        "prepare(function, &mut state)",
        "parameters.len() != template.parameters().len()",
        "preflight(template.parameters()",
        "cost.depth > limits.source_cost.max_depth",
        "cost.nodes > limits.source_cost.max_nodes",
        "register(function, body, &mut state)",
        "Ok(state)",
    ];
    for pair in gates.windows(2) {
        assert!(assembly.find(pair[0]).unwrap() < assembly.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        ".collect",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "catch_unwind",
        "RefCell",
        "Mutex",
        "RwLock",
        "Arc<",
        "BTreeMap",
        "fn fresh_name",
        "std::mem::take",
        "State: Clone",
        "State: Send",
        "Lowering: std::fmt",
        "Registration: std::fmt",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected registry coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_registry;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let assembly = reference
        .split_once("crate::program_source::assemble_declared_program(")
        .unwrap()
        .1
        .split_once("let (main, lowered, _functions)")
        .unwrap()
        .0;
    for boundary in [
        "|function, functions|",
        "crate::helper_body_source::lower_declared_helper_body(",
        "|function, parameters, visited|",
        "computation::lower_expression_with_functions(",
        "|function, admitted, functions|",
        ".insert(function.name.clone(), Template { prepared: admitted })",
        "HelperRegistryError::Dependency(error) => dependency_error(error, span)",
        "HelperRegistryError::Lowering { error, .. } => error",
        "HelperRegistryError::Registration { error, .. } => match error {}",
    ] {
        assert!(
            assembly.contains(boundary),
            "missing product registry delegation {boundary}"
        );
    }
    assert!(
        reference.contains("let assembled = crate::program_source::assemble_declared_program(")
    );
    assert!(
        reference.contains("computation::lower_computation_with_functions(&main.body, functions)")
    );
    assert!(!reference.contains("for function in order"));
    assert!(!reference.contains("helper_dependency_order("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_registry.rs")).unwrap();
    for evidence in [
        "borrowed_ready_order_registers_each_helper_before_the_next_without_compiling_the_entry",
        "every_dependency_template_and_cost_ceiling_precedes_native_callbacks",
        "whole_helper_source_and_cold_entry_calls_fail_before_any_preparation",
        "current_parameter_and_unused_prefix_limits_precede_the_selected_native_prepare",
        "prepared_signatures_cannot_substitute_reorder_rename_or_change_domains_before_registration",
        "current_template_and_cached_cost_bounds_precede_native_storage_with_depth_first_priority",
        "original_body_boxes_signature_result_buffers_and_cost_move_into_native_storage_once",
        "ready_native_failure_precedes_a_later_cycle_without_eager_order_collection",
        "late_cycle_drops_already_registered_owned_state_without_returning_a_partial_registry",
        "later_native_errors_and_unwind_drop_state_and_inputs_without_retry_refund_or_payload_formatting",
        "empty_registry_returns_original_state_under_explicit_zero_policies_without_callbacks",
        "helper_count_limits_and_unknown_calls_do_not_grant_native_lookup_or_skip_unused_helpers",
        "unrelated_scalar_registry_composes_declarations_bodies_instances_and_entry_with_exact_fuel",
        "reference_registry_delegation_preserves_nested_helper_wire_authority_and_ready_error_precedence",
        "std::ptr::eq",
        "as_ptr()",
        "catch_unwind",
        "fuel.remaining(), 97",
    ] {
        assert!(
            proof.contains(evidence),
            "missing registry proof {evidence}"
        );
    }
}

#[test]
fn shared_program_assembly_owns_complete_state_entry_and_mandatory_final_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/program_source.rs"))
            .unwrap();
    for boundary in [
        "pub struct ProgramSourceLimits",
        "pub enum ProgramSourceError<Lowering, Registration, Entry, Admission>",
        "pub struct ProgramSource<'source, Output, State>",
        "entry: &'source Function",
        "output: Output",
        "state: State",
        "pub fn into_parts(self) -> (&'source Function, Output, State)",
        "pub fn assemble_program<",
        "pub fn assemble_declared_program<",
        "declarations: &HelperDeclarations<'source>",
        "lower_entry: impl FnOnce",
        "admit: impl FnOnce",
        "&Output, &mut State",
        "max_source_nodes: limits.max_entry_source_nodes",
        "max_source_depth: limits.max_entry_source_depth",
        "max_arguments: limits.max_entry_arguments",
        "preflight::<Entry>(&declarations.entry().body, source)",
        "Native output is opaque: the host MUST",
        "not transactional rollback of external storage",
        "State may move between assembly stages",
        "no stable-address or pinning promise",
        "not a complete generic recursive compiler or independent VM",
    ] {
        assert!(
            source.contains(boundary),
            "missing program assembly boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn assemble_program<")
        .unwrap()
        .1
        .split_once("pub fn assemble_declared_program<")
        .unwrap()
        .0;
    assert!(
        public.find("!valid_limits(source)").unwrap() < public.find("preflight::<Entry>").unwrap()
    );
    assert!(
        public.find("preflight::<Entry>").unwrap()
            < public.find("assemble_preflighted_program(").unwrap()
    );
    let assembly = source
        .split_once("pub fn assemble_declared_program<")
        .unwrap()
        .1;
    let gates = [
        "!valid_limits(entry_limits(limits))",
        "assemble_helper_registry(declarations, limits.helpers, state, prepare, register)",
        ".map_err(ProgramSourceError::Helpers)",
        "let entry = declarations.entry()",
        "lower_entry(entry, &mut state)",
        "admit(entry, &output, &mut state)",
        "Ok(ProgramSource",
    ];
    for pair in gates.windows(2) {
        assert!(assembly.find(pair[0]).unwrap() < assembly.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        ".collect",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "catch_unwind",
        "RefCell",
        "Mutex",
        "RwLock",
        "Arc<",
        "BTreeMap",
        "State: Clone",
        "Output: Clone",
        "Output: Send",
        "std::mem::take",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected program assembly coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod program_source;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    let assembly = reference
        .split_once("let assembled = crate::program_source::assemble_declared_program(")
        .unwrap()
        .1
        .split_once("let (main, lowered, _functions) = assembled.into_parts()")
        .unwrap()
        .0;
    for boundary in [
        "helpers: crate::helper_registry::HelperRegistryLimits",
        "|function, functions|",
        "|function, admitted, functions|",
        "|main, functions|",
        "computation::lower_computation_with_functions(&main.body, functions)",
        "|main, lowered, _|",
        "canonical_source(&lowered.effect)",
        "ProgramSourceError::Helpers(error)",
        "ProgramSourceError::Entry { error, .. }",
        "ProgramSourceError::Admission { error, .. }",
    ] {
        assert!(
            assembly.contains(boundary),
            "missing reference program assembly {boundary}"
        );
    }
    assert!(
        assembly.find("|function, admitted, functions|").unwrap()
            < assembly.find("|main, functions|").unwrap()
    );
    assert!(
        assembly.find("|main, functions|").unwrap() < assembly.find("|main, lowered, _|").unwrap()
    );
    assert_eq!(
        assembly
            .matches("canonical_source(&lowered.effect)")
            .count(),
        1
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/program_source.rs")).unwrap();
    for evidence in [
        "original_custom_entry_runs_after_every_ready_helper_then_whole_output_admission_once",
        "all_entry_source_ceilings_precede_helper_entry_and_admission_callbacks",
        "cold_entry_names_text_arity_frontier_and_depth_fail_before_any_native_helper_work",
        "current_helper_ceilings_and_dependency_errors_never_reach_the_entry",
        "late_cycle_drops_completed_helper_state_without_entry_lowering_or_partial_program",
        "ready_helper_native_errors_keep_original_payload_span_and_precede_later_cycle",
        "entry_error_and_unwind_drop_owned_registry_without_admission_retry_or_counter_refund",
        "final_admission_error_and_unwind_drop_original_output_and_state_without_partial_handoff",
        "once_entry_and_admission_closures_move_private_buffers_into_and_out_of_original_state",
        "empty_helper_zero_policies_still_lower_and_admit_a_parameterless_leaf_entry",
        "zero_entry_nodes_reject_before_helpers_even_when_the_helper_policy_is_empty",
        "unknown_entry_calls_and_stale_native_output_need_explicit_fresh_host_policy",
        "unrelated_scalar_program_composes_registry_entry_admission_and_value_with_exact_fuel",
        "reference_program_assembly_keeps_canonical_wire_capabilities_and_helper_before_entry_errors",
        "std::ptr::eq",
        "as_ptr()",
        "catch_unwind",
        "fuel.remaining(), 97",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(evidence),
            "missing program assembly proof {evidence}"
        );
    }
}

#[test]
fn shared_helper_declaration_admission_borrows_complete_forests_without_product_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/helper_declarations.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperDeclarationLimits",
        "pub enum HelperDeclarationError<Error>",
        "pub struct HelperDeclarations<'source>",
        "entry: &'source Function",
        "helpers: Vec<&'source Function>",
        "reserved_names: HashSet<&'source str>",
        "pub fn accept_helper_declarations<'source, Error>",
        "mut reserved: impl FnMut(&str) -> Result<bool, Error>",
        "limits.max_functions > MAX_FUNCTIONS",
        "limits.max_parameters > MAX_FUNCTION_PARAMETERS",
        "limits.max_source_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_source_depth > MAX_TYPE_INFERENCE_DEPTH",
        "declarations.len() > limits.max_functions",
        "!valid_local_name(entry)",
        "helper_parameters(function, limits.max_parameters)",
        "budget.visit(depth, 0, 0)",
        "arguments.len()",
        "callee == \"fold\"",
        "argument.name == \"item\"",
        "names.insert(value)",
        "names.insert(name)",
        "!entry.parameters.is_empty()",
        "Self::Policy { .. } => \"native declaration name policy failed\"",
        "No names, AST nodes or literal buffers are copied",
    ] {
        assert!(
            source.contains(boundary),
            "missing declaration boundary {boundary}"
        );
    }
    let admission = source
        .split_once("pub fn accept_helper_declarations")
        .unwrap()
        .1;
    let policy = admission.find("reserved(&function.name)").unwrap();
    for check in [
        "limits.max_functions",
        "declarations.len()",
        "!valid_local_name(entry)",
        "!valid_local_name(&function.name)",
    ] {
        assert!(admission.find(check).unwrap() < policy);
    }
    let gates = [
        "reserved(&function.name)",
        "declarations[..declaration_index]",
        "helper_parameters(function",
        "collect_source_names(function",
        "let entry_index",
        "!entry.parameters.is_empty()",
        "Ok(HelperDeclarations",
    ];
    for pair in gates.windows(2) {
        assert!(admission.find(pair[0]).unwrap() < admission.find(pair[1]).unwrap());
    }
    let scan = source
        .split_once("fn collect_source_names")
        .unwrap()
        .1
        .split_once("/// Admit headers")
        .unwrap()
        .0;
    assert!(scan.find(".check_pending(").unwrap() < scan.find("names.extend(").unwrap());
    assert!(scan.find(".check_pending(").unwrap() < scan.find("pending.extend(").unwrap());
    for coupling in [
        ".clone()",
        "to_owned()",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "infer_pure_type",
        "evaluate_pure",
        "canonical_source",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "fn fresh_name",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected declaration coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_declarations;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/functions.rs")).unwrap();
    for boundary in [
        "crate::helper_declarations::accept_helper_declarations(",
        "computation::is_builtin(name)",
        "accepted.reserved_names().map(str::to_owned)",
        "let (main, lowered, _functions) = assembled.into_parts()",
        "crate::program_source::assemble_declared_program(",
        "parameter_diagnostics(error, span)",
    ] {
        assert!(
            reference.contains(boundary),
            "missing declaration adapter {boundary}"
        );
    }
    assert!(!reference.contains("let mut declared = BTreeMap"));
    assert!(!reference.contains("let mut pending = vec![(&function.body"));
    assert!(
        reference.find("accept_helper_declarations(").unwrap()
            < reference.find("assemble_declared_program(").unwrap()
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_declarations.rs"))
            .unwrap();
    for boundary in [
        "custom_entry_retains_original_declarations_source_order_and_borrowed_buffers",
        "invalid_ceilings_count_and_entry_reject_before_any_native_policy",
        "reserved_policy_is_explicit_once_per_valid_header_and_precedes_duplicate_check",
        "signature_errors_keep_original_parameter_spans_and_precede_physical_body_errors",
        "missing_and_parameterized_entry_checks_follow_the_complete_cold_forest",
        "cold_frontier_is_bounded_before_name_inventory_or_child_stack_growth",
        "reservation_inventory_includes_cold_fold_labels_but_not_callees_or_literals",
        "native_policy_failure_and_unwind_return_no_partial_view_or_retry",
        "admission_does_not_certify_native_calls_entry_cycles_literals_or_cold_types",
        "accepted_borrowed_headers_compose_with_shared_templates_hygiene_and_exact_fuel",
        "reference_declaration_diagnostics_wire_names_and_cold_authority_are_unchanged",
        "std::ptr::eq",
        "as_ptr()",
        "catch_unwind",
        "HelperTemplate::new",
        "hygienic_helper_body",
        "bind_helper_arguments",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing declaration proof {boundary}"
        );
    }
}

#[test]
fn native_graph_inspection_borrows_original_slots_and_stays_separate_from_native_typing() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/native_graph.rs")).unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for boundary in [
        "pub struct NativeGraphLimits",
        "pub enum NativeGraphShape<'graph, Branch>",
        "members: &'graph [Branch]",
        "pub struct NativeGraphSummary",
        "pub enum NativeGraphPhase",
        "pub enum NativeGraphError<Error>",
        "Node: ?Sized + 'graph",
        "limits.max_nodes > MAX_NATIVE_GRAPH_NODES",
        "limits.max_depth > MAX_NATIVE_GRAPH_DEPTH",
        "limits.max_members > MAX_NATIVE_GRAPH_MEMBERS",
        "let node_index = budget.visited()",
        "frames.last_mut()",
        "frame.members.get(frame.next)",
        "NativeGraphPhase::View",
        "NativeGraphPhase::Child",
        "NativeGraphPhase::Leaf",
        "No quota silently expands",
        "one borrowed sibling cursor per open group",
        "this entry does not promise that all physical errors precede inline leaf work",
        "without retries or partial summary",
    ] {
        assert!(
            source.contains(boundary),
            "missing graph boundary {boundary}"
        );
    }
    assert!(
        compact
            .contains("pubfninspect_native_graph<'graph,Node:?Sized+'graph,Branch:'graph,Error>")
    );
    assert!(compact.contains("letminimum=ifkind==GroupKind::Sequence{1}else{2}"));
    let inspection = source
        .split_once("pub fn inspect_native_graph")
        .unwrap()
        .1
        .split_whitespace()
        .collect::<String>();
    assert!(inspection.find("budget.visit(").unwrap() < inspection.find("view(node)").unwrap());
    assert!(
        inspection.find("limits.max_members).contains").unwrap()
            < inspection.find("child(member)").unwrap()
    );
    assert!(inspection.find("leaf(node)").unwrap() < inspection.find("child(member)").unwrap());
    for coupling in [
        ".clone()",
        "pending.extend",
        "serde::",
        "use serde",
        "crate::Type",
        "HostOperation",
        "OperationCatalog",
        "ResultField",
        "infer_pure_type",
        "evaluate_pure",
        "canonical_source",
        "Fuel::",
        "Send +",
        "PartialEq +",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected graph coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod native_graph;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/leserpent_profile.rs"))
            .unwrap();
    let adapter = reference
        .split_once("fn validate_canonical_effect_shape")
        .unwrap()
        .1
        .split_once("fn canonical_effect_source")
        .unwrap()
        .0;
    for boundary in [
        "inspect_native_graph(",
        "max_nodes: MAX_CANONICAL_EFFECT_NODES",
        "max_depth: MAX_EFFECT_NESTING_DEPTH - 1",
        "max_members: MAX_ALL_BRANCHES",
        "ir::GroupKind::Parallel",
        "ir::GroupKind::Sequence",
        "|branch: &HirBranch| Ok(&branch.effect)",
        "computation::validate_shape(expression)",
        "\"LSH1201\"",
        "\"LSH1204\"",
        "\"LSH1205\"",
        "NativeGraphError::Native { error, .. } => error",
        "span: None",
    ] {
        assert!(
            adapter.contains(boundary),
            "missing product graph adapter {boundary}"
        );
    }
    assert!(!adapter.contains("pending"));
    assert!(!adapter.contains("StructureBudget::new"));
    assert!(reference.contains(
        "shared_native_walk_preserves_inline_computation_error_before_later_group_errors"
    ));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_graph.rs")).unwrap();
    for boundary in [
        "nested_gui_graphs_borrow_original_nonclone_slices_slots_and_declaration_order",
        "node_depth_and_member_limits_are_inclusive_and_zero_has_no_expanding_default",
        "invalid_group_arity_stops_before_any_original_child_mapping",
        "repeated_original_edges_count_per_occurrence_without_deduplication",
        "cyclic_native_views_are_bounded_without_recursive_calls_or_cycle_certificates",
        "private_native_errors_keep_positions_and_phase_but_never_format_the_payload",
        "native_callback_unwind_never_retries_consumes_graphs_or_publishes_partial_counts",
        "inline_leaf_checks_preserve_first_failure_without_claiming_whole_cold_admission",
        "successful_counts_do_not_certify_changed_graphs_leaf_payloads_or_live_grants",
        "depth_before_nodes_and_original_payload_free_debug_are_not_native_formatters",
        "unsized_gui_local_nodes_and_nonclone_branch_slots_need_no_box_conversion",
    ] {
        assert!(proof.contains(boundary), "missing graph proof {boundary}");
    }
    let product_proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_graph_reference.rs"))
            .unwrap();
    for boundary in [
        "existing_nested_native_parallel_profiles_keep_canonical_wire_order_and_capability_checks",
        "physical_native_graph_counts_do_not_certify_names_types_or_private_payload_domains",
        "shared_structure_inspection_does_not_enable_nested_exports_or_forbidden_sequence_profiles",
        "compute_leaf_structure_stays_separate_from_its_semantic_type_and_native_graph_budget",
        "graph_arity_and_depth_keep_legacy_diagnostics_before_source_formatting",
    ] {
        assert!(
            product_proof.contains(boundary),
            "missing product graph proof {boundary}"
        );
    }
}

#[test]
fn closed_native_member_admission_is_borrowed_bounded_and_used_by_product_and_gui_adapters() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/native_group.rs")).unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for marker in [
        "pub struct NativeGroupMembers<'group, Branch>",
        "members: &'group [Branch]",
        "names: [Option<&'group str>; MAX_NATIVE_GRAPH_MEMBERS]",
        "pub enum NativeGroupMemberPhase",
        "pub enum NativeGroupMemberError<Error>",
        "max_members > MAX_NATIVE_GRAPH_MEMBERS",
        "GroupKind::Sequence => 1",
        "GroupKind::Parallel => 2",
        "let mut names = [None; MAX_NATIVE_GRAPH_MEMBERS]",
        "valid_member_name(observed)",
        "names[..member_index].contains(&Some(observed))",
        "NativeGroupMemberPhase::Name",
        "NativeGroupMemberPhase::Admission",
        "member_index",
        "not infer those facts",
        "No partially",
        "no hook is retried",
        "External native work is not rolled back",
    ] {
        assert!(
            source.contains(marker),
            "missing native member boundary {marker}"
        );
    }
    let entry = compact
        .split_once("pubfnadmit_native_group_members")
        .unwrap()
        .1
        .split_once("pub(crate)fnpreflight_native_group_members")
        .unwrap()
        .0;
    assert!(
        entry.find("preflight_native_group_members(").unwrap()
            < entry.find("admit(member)").unwrap()
    );
    let preflight = compact
        .split_once("pub(crate)fnpreflight_native_group_members")
        .unwrap()
        .1;
    let ordered = [
        "max_members>MAX_NATIVE_GRAPH_MEMBERS",
        "minimum..=max_members",
        "name(member)",
        "valid_member_name(observed)",
        "names[..member_index].contains",
        "names[member_index]=Some(observed)",
        "Ok(NativeGroupMembers",
    ];
    for pair in ordered.windows(2) {
        assert!(preflight.find(pair[0]).unwrap() < preflight.find(pair[1]).unwrap());
    }
    for coupling in [
        "Vec<",
        "Box<",
        ".clone()",
        "serde::",
        "crate::Type",
        "HostOperation",
        "OperationCatalog",
        "ResultField",
        "Fuel::",
        "evaluate_pure",
        "infer_pure_type",
        "canonical_source",
        "Send +",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected native member coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod native_group;"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    let gate = adapter
        .split_once("pub(crate) fn flat_native_group")
        .unwrap()
        .1
        .split_once("pub(crate) fn supports_host_flow")
        .unwrap()
        .0;
    assert!(gate.contains("crate::native_group::admit_native_group_members("));
    assert!(gate.contains("operation.schema().result.ty != branch.result_type"));
    assert!(gate.contains("admitted.kind(), admitted.members()"));
    assert!(!gate.contains("valid_member_name"));
    assert!(!gate.contains("branches[..index]"));
    assert!(adapter.contains(
        "shared_native_member_gate_keeps_original_label_limits_and_requires_prior_payload_admission"
    ));
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_flow.rs"))
        .unwrap();
    assert!(gui.contains("leselang_hir::native_group::admit_native_group_members("));
    assert!(gui.contains("members.len() < self.max_native_nodes.get()"));
    assert!(gui.contains("self.atomic_valid(&member.effect)"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group.rs")).unwrap();
    for marker in [
        "original_nonclone_names_branch_slots_and_modes_are_borrowed_in_declaration_order",
        "inclusive_arity_zero_and_safety_ceiling_deny_before_any_hook",
        "all_names_are_checked_before_any_native_admission_even_if_first_native_row_is_invalid",
        "labels_follow_member_not_lexical_grammar_and_original_buffers_never_copy",
        "duplicate_equal_but_distinct_native_buffers_are_rejected_without_native_queries",
        "native_name_and_admission_errors_keep_original_phase_index_payload_without_formatting",
        "native_unwind_stops_once_keeps_graphs_and_never_returns_partial_views",
        "current_native_policy_type_identity_and_atomic_eligibility_are_not_inferred_by_the_core",
        "successful_view_does_not_certify_changed_live_policy_or_hidden_payload_limits",
        "callback_observed_names_are_ephemeral_native_metadata_not_automatic_authenticity",
        "shared_member_admission_composes_with_graph_preflight_but_never_replaces_its_budget",
        "unsized_gui_operation_views_and_move_only_branches_need_no_box_conversion_or_thread_transfer",
    ] {
        assert!(
            proof.contains(marker),
            "missing native member proof {marker}"
        );
    }
}

#[test]
fn fresh_native_export_assembly_is_shared_bounded_and_requires_complete_final_native_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/native_group_exports.rs"))
            .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for marker in [
        "pub enum NativeGroupExportPhase",
        "Operation { member_index: usize }",
        "Signature",
        "pub enum NativeGroupExportError<Error>",
        "Members(NativeGroupMemberError<Error>)",
        "impl<Error> std::error::Error for NativeGroupExportError<Error>",
        "pub fn observe_native_group_exports<'group, Branch, Operation, Error>",
        "members: &'group [Branch]",
        "No prior NativeGroupMembers view or cached grant is accepted",
        "not inferred by the core",
        "No partially observed exports escape",
        "hooks never retry",
        "Drop may itself unwind",
        "mutable ephemeral GroupExports",
    ] {
        assert!(
            source.contains(marker),
            "missing native export boundary {marker}"
        );
    }
    for marker in [
        "name:implFnMut(&'groupBranch)->Result<&'groupstr,Error>",
        "mutoperation:implFnMut(&'groupBranch)->Result<Operation,Error>",
        "admit:implFnOnce(GroupKind,&'group[Branch],&[GroupExport<'group,Operation>],)->Result<(),Error>",
    ] {
        assert!(
            compact.contains(marker),
            "missing native export hook {marker}"
        );
    }
    let ordered = [
        "preflight_native_group_members(kind,members,max_members,name)",
        "Vec::with_capacity(members.len())",
        "observed.name(member_index)",
        "operation(member)",
        "exports.push(GroupExport{name,operation})",
        "admit(observed.kind(),observed.members(),&exports)",
        "Ok(GroupExports",
    ];
    for pair in ordered.windows(2) {
        assert!(compact.find(pair[0]).unwrap() < compact.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        "serde::",
        "crate::Type",
        "HostOperation",
        "ResultField",
        "Fuel::",
        "evaluate_pure",
        "infer_pure_type",
        "canonical_source",
        "Send +",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected native export coupling {coupling}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod native_group_exports;"));
    let entry = compact
        .split_once("pubfnobserve_native_group_exports")
        .unwrap()
        .1;
    for abrupt in ["panic!(", "expect(", "unwrap("] {
        assert!(
            !entry.contains(abrupt),
            "abrupt native export path {abrupt}"
        );
    }
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    let gate = adapter
        .split_once("pub(crate) fn native_group_exports")
        .unwrap()
        .1
        .split_once("impl<'schema>")
        .unwrap()
        .0;
    for marker in [
        "crate::Effect::Sequence",
        "crate::Effect::All",
        "_ => return None",
        "crate::native_group_exports::observe_native_group_exports(",
        "crate::MAX_ALL_BRANCHES",
        "HostOperation::for_effect(&branch.effect).ok_or(())?",
        "operation.schema().result.ty == branch.result_type",
        "candidate_kind == kind",
        "original.len() == exports.len()",
        "std::ptr::eq(branch.name.as_str(), export.name)",
        "HostOperation::for_effect(&branch.effect) == Some(export.operation)",
        "export.operation.schema().result.ty == branch.result_type",
    ] {
        assert!(
            gate.contains(marker),
            "missing product native export gate {marker}"
        );
    }
    assert!(!gate.contains("canonical_source"));
    assert!(!gate.contains("valid_member_name"));
    assert!(adapter.contains("let exports = native_group_exports(effect)?;"));
    assert!(adapter.contains("&member.operation.schema().result.operation"));
    assert!(adapter.contains(
        "shared_native_exports_keep_original_order_canonical_rows_and_closed_flat_profile"
    ));
    let computation =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let signature = computation
        .split_once("fn group_signature(")
        .unwrap()
        .1
        .split_once("struct BindingValue")
        .unwrap()
        .0;
    assert!(signature.contains("crate::pure_reference::native_group_exports(effect).ok_or(())"));
    assert!(!signature.contains("flat_native_group(effect)"));
    assert!(!signature.contains("HostOperation::for_effect(&branch.effect)"));
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_flow.rs"))
        .unwrap();
    for marker in [
        "leselang_hir::native_group_exports::observe_native_group_exports(",
        "self.version.get() == 9",
        "self.granted.get()",
        "original.len() < self.max_native_nodes.get()",
        "std::ptr::eq(member.name.as_str(), observed.name)",
        "std::ptr::eq(operation, observed.operation.operation)",
        "std::ptr::eq(member.declaration, &operation.row.result)",
    ] {
        assert!(
            gui.contains(marker),
            "missing independent GUI export gate {marker}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_exports.rs"))
            .unwrap();
    for marker in [
        "fresh_names_original_branch_order_and_move_only_observations_reach_one_complete_signature",
        "current_limits_arity_invalid_and_duplicate_names_precede_any_native_operation",
        "opaque_name_and_operation_errors_keep_original_positions_without_private_formatting",
        "late_signature_rejection_drops_every_observation_without_partial_output_or_graph_consumption",
        "final_native_admission_corroborates_original_name_row_type_atomicity_and_current_policy",
        "callbacks_and_signature_unwind_stop_without_retry_and_release_owned_prefixes",
        "cleanup_unwind_does_not_retry_signature_or_consume_borrowed_native_members",
        "every_entry_rechecks_current_names_and_limits_without_accepting_a_cached_member_view",
        "signature_admission_is_once_owned_and_success_is_mutable_metadata_not_authority",
        "unrelated_unsized_gui_and_opaque_device_schemas_need_no_native_clone_equality_or_wire_traits",
        "explicit_native_noop_admission_is_not_an_automatic_row_or_name_authenticity_proof",
    ] {
        assert!(
            proof.contains(marker),
            "missing native export proof {marker}"
        );
    }
}

#[test]
fn fresh_closed_signature_comparison_borrows_both_sides_before_explicit_native_identity_pairs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/native_group_signature.rs"),
    )
    .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for marker in [
        "pub enum NativeGroupSignatureSide",
        "Left",
        "Right",
        "pub enum NativeGroupSignatureError<Error>",
        "error: NativeGroupMemberError<Error>",
        "impl<Error> std::error::Error for NativeGroupSignatureError<Error>",
        "pub fn compare_native_group_signatures<'left, 'right, Left, Right, Error>",
        "No cached member view, derived equality or pointer shortcut",
        "false/error/unwind stops without retry",
        "comparator may bridge different native member/schema representations",
        "Borrowed graphs are not consumed",
        "True is ephemeral metadata",
    ] {
        assert!(
            source.contains(marker),
            "missing signature boundary {marker}"
        );
    }
    for marker in [
        "left:(GroupKind,&'left[Left])",
        "right:(GroupKind,&'right[Right])",
        "left_name:implFnMut(&'leftLeft)->Result<&'leftstr,Error>",
        "right_name:implFnMut(&'rightRight)->Result<&'rightstr,Error>",
        "mutsame:implFnMut(&'leftLeft,&'rightRight)->Result<bool,Error>",
        "(Some(left),Some(right))=>left!=right",
        "_=>true",
    ] {
        assert!(
            compact.contains(marker),
            "missing borrowed signature gate {marker}"
        );
    }
    let ordered = [
        "preflight_native_group_members(left.0,left.1,max_members,left_name)",
        "preflight_native_group_members(right.0,right.1,max_members,right_name)",
        "left.kind()!=right.kind()",
        "left.members().len()!=right.members().len()",
        "left.name(index),right.name(index)",
        "same(left,right)",
        "Ok(true)",
    ];
    for pair in ordered.windows(2) {
        assert!(compact.find(pair[0]).unwrap() < compact.find(pair[1]).unwrap());
    }
    for coupling in [
        "Vec<",
        "Box<",
        ".clone()",
        "serde::",
        "crate::Type",
        "HostOperation",
        "ResultField",
        "Fuel::",
        "canonical_source",
        "infer_pure_type",
        "Send +",
        "Mutex",
        "std::ptr::eq",
        "ptr_eq(",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected signature coupling {coupling}"
        );
    }
    let entry = compact
        .split_once("pubfncompare_native_group_signatures")
        .unwrap()
        .1;
    for abrupt in ["panic!(", "expect(", "unwrap("] {
        assert!(!entry.contains(abrupt), "abrupt signature path {abrupt}");
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod native_group_signature;"));
    let exports =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/group_exports.rs"))
            .unwrap();
    assert!(exports.contains("crate::native_group_signature::compare_native_group_signatures("));
    let export_compact = exports
        .split_whitespace()
        .collect::<String>()
        .replace(",}", "}");
    assert!(export_compact.contains("NativeGroupSignatureError::Native{member_index,error}"));
    assert!(export_compact.contains("phase:GroupExportPhase::Compare{index:member_index}"));
    assert!(exports.contains("same(&expected.operation, &current.operation)"));
    assert!(!exports.contains("expected.kind != current.kind"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    let equality = adapter
        .split_once("impl PartialEq for ReferenceMembers")
        .unwrap()
        .1
        .split_once("struct ReferenceResult")
        .unwrap()
        .0;
    for marker in [
        "Self::External(left), Self::External(right)",
        "left == right",
        "crate::native_group_signature::compare_native_group_signatures(",
        "left.as_ref()",
        "right.as_ref()",
        "crate::MAX_ALL_BRANCHES",
        "|left, right| Ok(left.1 == right.1)",
        "External legacy metadata has no mode",
        ".unwrap_or(false)",
        "_ => false",
    ] {
        assert!(
            equality.contains(marker),
            "missing product signature gate {marker}"
        );
    }
    assert!(!equality.contains("ptr_eq("));
    assert!(adapter.contains(
        "shared_signature_comparison_preserves_product_tags_and_legacy_external_variant_identity"
    ));
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_flow.rs"))
        .unwrap();
    assert!(gui.contains("leselang_hir::native_group_signature::compare_native_group_signatures("));
    assert!(gui.contains("std::ptr::eq(left.operation.row, right.operation.row)"));
    assert!(gui.contains("std::ptr::eq(left.declaration, right.declaration)"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_signature.rs"))
            .unwrap();
    for marker in [
        "both_complete_once_borrowed_names_precede_original_pairs_in_declaration_order",
        "valid_kind_count_and_name_order_mismatches_never_enter_native_comparison",
        "limits_and_invalid_members_report_original_side_before_any_native_pair",
        "opaque_name_errors_keep_side_index_and_payload_without_formatting_or_sources",
        "false_and_opaque_pair_errors_stop_once_at_the_original_member_index",
        "native_name_and_pair_unwind_stop_without_retry_or_consuming_borrowed_members",
        "fresh_current_names_limits_and_live_row_policy_cannot_reuse_a_prior_true_result",
        "aliased_slices_still_query_both_sides_and_every_current_native_pair",
        "heterogeneous_unsized_gui_and_device_members_need_no_native_clone_equality_or_wire_traits",
        "explicit_native_comparison_is_not_automatic_schema_identity_or_future_authority",
    ] {
        assert!(
            proof.contains(marker),
            "missing closed signature proof {marker}"
        );
    }
}

#[test]
fn fresh_closed_member_lookup_preflights_whole_names_before_once_original_selection() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/native_group_lookup.rs"))
            .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for marker in [
        "pub enum NativeGroupLookupError<Error>",
        "InvalidQuery",
        "Members(NativeGroupMemberError<Error>)",
        "impl<Error> std::error::Error for NativeGroupLookupError<Error>",
        "pub fn lookup_native_group_member<'group, Member, Error>",
        "duplicate tails after an otherwise matching member",
        "Missing names return None",
        "FnOnce native hook",
        "No cached admitted view",
        "not a reusable type/grant/receipt/fuel",
        "not sandboxed or preempted",
        "callback/cleanup effects cannot be rolled back",
        "hook state is dropped normally even when there is no match",
    ] {
        assert!(
            source.contains(marker),
            "missing member lookup boundary {marker}"
        );
    }
    for marker in [
        "members:&'group[Member]",
        "max_members:usize",
        "query_name:&str",
        "name:implFnMut(&'groupMember)->Result<&'groupstr,Error>",
        "admit:implFnOnce(&'groupMember)->Result<bool,Error>",
        "Result<Option<&'groupMember>,NativeGroupLookupError<Error>>",
        "map_err(NativeGroupLookupError::Members)?",
        "NativeGroupLookupError::Native{member_index,error",
    ] {
        assert!(
            compact.contains(marker),
            "missing borrowed lookup gate {marker}"
        );
    }
    let entry = compact
        .split_once("pubfnlookup_native_group_member")
        .unwrap()
        .1;
    let ordered = [
        "max_members>MAX_NATIVE_GRAPH_MEMBERS",
        "!valid_member_name(query_name)",
        "preflight_native_group_members(kind,members,max_members,name)",
        "observed.members().iter().enumerate()",
        "observed.name(member_index)==Some(query_name)",
        "returnadmit(member)",
        "accepted.then_some(member)",
        "Ok(None)",
    ];
    for pair in ordered.windows(2) {
        assert!(entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap());
    }
    for coupling in [
        "Vec<",
        "Box<",
        ".clone()",
        "serde::",
        "crate::Type",
        "HostOperation",
        "ResultField",
        "Fuel::",
        "canonical_source",
        "infer_pure_type",
        "Send +",
        "Mutex",
        "std::ptr::eq",
        "PartialEq",
        "dispatch(",
        "evaluate(",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected member lookup coupling {coupling}"
        );
    }
    for abrupt in ["panic!(", "expect(", "unwrap("] {
        assert!(
            !entry.contains(abrupt),
            "abrupt member lookup path {abrupt}"
        );
    }
    let modules =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod native_group_lookup;"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/pure_reference.rs")).unwrap();
    let product = adapter
        .split_once("fn member_result(")
        .unwrap()
        .1
        .split_once("fn join_results(")
        .unwrap()
        .0;
    let external = product.split_once("ReferenceMembers::Computed").unwrap().0;
    assert!(external.contains("ReferenceMembers::External(members)"));
    assert!(
        external.contains(".any(|(member, declared)| member == name && declared == operation)")
    );
    for marker in [
        "crate::native_group_lookup::lookup_native_group_member(",
        "*kind",
        "members.as_ref()",
        "crate::MAX_ALL_BRANCHES",
        "|member| -> Result<_, ()> { Ok(member.0) }",
        "|member| Ok(member.1 == operation)",
        ".ok()",
        ".flatten()",
        ".is_some()",
        "ty: operation.result_type()",
        "members: None",
    ] {
        assert!(
            product.contains(marker),
            "missing product member lookup rule {marker}"
        );
    }
    assert!(
        adapter.contains(
            "shared_member_lookup_keeps_closed_operation_tags_and_legacy_external_queries"
        )
    );
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_flow.rs"))
        .unwrap();
    let gui = gui
        .split_once("fn member_result(")
        .unwrap()
        .1
        .split_once("fn join_results(")
        .unwrap()
        .0;
    for marker in [
        "self.trace.record(\"member\")",
        "leselang_hir::native_group_lookup::lookup_native_group_member(",
        "|member| -> Result<_, ()> { Ok(member.name) }",
        "|member| Ok(std::ptr::eq(member.operation.row, operation.row))",
        ".ok()??",
        "self.metadata(Query::Atomic(member.declaration))",
    ] {
        assert!(
            gui.contains(marker),
            "missing GUI member lookup rule {marker}"
        );
    }
    assert!(
        gui.split_whitespace()
            .collect::<String>()
            .contains("*kind,members.as_ref(),4,name,")
    );
    assert!(!gui.contains(".clone()"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_lookup.rs"))
            .unwrap();
    for marker in [
        "complete_once_borrowed_names_precede_one_original_selected_member_admission",
        "invalid_ceiling_query_and_arity_deny_before_name_or_selected_native_hooks",
        "invalid_or_duplicate_tail_cannot_hide_behind_an_earlier_matching_name",
        "missing_names_never_query_native_rows_and_rejection_never_falls_back_to_another_slot",
        "opaque_name_and_selected_errors_keep_original_phase_index_and_redacted_payloads",
        "native_name_and_admission_unwind_stop_without_retry_or_consuming_borrowed_members",
        "once_owned_admission_state_cleans_up_even_without_invocation_or_on_unwind",
        "prior_selection_does_not_certify_changed_names_limits_or_current_native_policy",
        "inclusive_ceiling_shared_label_grammar_and_unsized_gui_local_slots_need_no_native_traits",
        "explicit_native_observations_are_not_automatic_original_name_row_or_grant_authenticity",
    ] {
        assert!(
            proof.contains(marker),
            "missing fresh closed member lookup proof {marker}"
        );
    }
}

#[test]
fn language_group_shape_preflight_reuses_native_names_without_new_admission_or_source_grammar() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let exports =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/group_exports.rs"))
            .unwrap();
    let shape = exports
        .split_once("fn member_shape")
        .unwrap()
        .1
        .split_once("pub(crate) fn physical")
        .unwrap()
        .0;
    let shape_compact = shape.split_whitespace().collect::<String>();
    for marker in [
        "members:&'name[Member]",
        "mutname:implFnMut(&'nameMember)->&'namestr",
        "preflight_native_group_members(kind,members,maximum,|member|",
        "Ok::<_,std::convert::Infallible>(name(member))",
        ".is_ok()",
    ] {
        assert!(
            shape_compact.contains(marker),
            "missing shared shape boundary {marker}"
        );
    }
    for parallel in [
        "Vec",
        ".clone(",
        ".bytes()",
        "count:",
        ".all(",
        "admit_native_group_members(",
    ] {
        assert!(
            !shape.contains(parallel),
            "parallel/export native shape implementation {parallel}"
        );
    }
    let physical = exports
        .split_once("pub(crate) fn physical")
        .unwrap()
        .1
        .split_once("pub(crate) fn atomic_candidate")
        .unwrap()
        .0;
    let compact = physical.split_whitespace().collect::<String>();
    assert!(compact.contains("member_shape(*group_kind,branches,limits.max_members,|branch|"));
    assert!(compact.contains("branch.name.as_str()"));
    let observe = exports
        .split_once("pub fn observe_group_exports")
        .unwrap()
        .1
        .split_once("#[cfg(test)]")
        .unwrap()
        .0;
    let compact = observe.split_whitespace().collect::<String>();
    assert!(
        compact.contains("member_shape(current.kind,&current.members,limits.max_members,|member|")
    );
    assert!(compact.contains("member.name"));
    let flow =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/flow_typing.rs")).unwrap();
    let flow_preflight = flow
        .split_once("fn preflight")
        .unwrap()
        .1
        .split_once("fn infer<")
        .unwrap()
        .0;
    let compact = flow_preflight
        .split_whitespace()
        .collect::<String>()
        .replace(",}", "}");
    for marker in [
        "preflight_native_group_members(*group_kind,branches,group_limits.max_branches,",
        "Ok::<_,std::convert::Infallible>(branch.name.as_str())",
        ".map(|_|())",
        "NativeGroupMemberError::InvalidName{member_index}",
        "GroupTypeError::InvalidName{branch_index:member_index}",
        "NativeGroupMemberError::DuplicateName{member_index}",
        "GroupTypeError::DuplicateName{branch_index:member_index}",
        "NativeGroupMemberError::Native{error,..}=>matcherror{}",
    ] {
        assert!(
            compact.contains(marker),
            "missing existing flow shape diagnostic {marker}"
        );
    }
    let ordered = [
        "group_index>=group_limits.max_groups",
        "preflight_native_group_members(",
        "letmutmembers=Vec::with_capacity(branches.len())",
        "preflight_atomic_leaves_with_budget(",
    ];
    for pair in ordered.windows(2) {
        assert!(compact.find(pair[0]).unwrap() < compact.find(pair[1]).unwrap());
    }
    let effect =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_evaluation.rs"))
            .unwrap();
    let effect_preflight = effect
        .split_once("pub(crate) fn preflight")
        .unwrap()
        .1
        .split_once("pub fn evaluate_effects_in_scope")
        .unwrap()
        .0;
    let compact = effect_preflight
        .split_whitespace()
        .collect::<String>()
        .replace(",}", "}");
    let group = compact
        .split_once("Computation::Group{group_kind,branches}=>{")
        .unwrap()
        .1
        .split_once("Computation::Choose")
        .unwrap()
        .0;
    for marker in [
        "preflight_native_group_members(*group_kind,branches,limits.max_branches,",
        "Ok::<_,std::convert::Infallible>(branch.name.as_str())",
        ".is_err()",
        "EffectEvaluationFault::GroupShape.into()",
    ] {
        assert!(
            group.contains(marker),
            "missing effect shape compatibility {marker}"
        );
    }
    assert!(
        group.find("preflight_native_group_members(").unwrap()
            < group.find("effects.push(node)").unwrap()
    );
    for pass in [flow_preflight, effect_preflight] {
        for duplicate in [
            "branch.name.is_empty()",
            "branch.name.bytes()",
            "branches[..",
            "admit_native_group_members(",
        ] {
            assert!(
                !pass.contains(duplicate),
                "parallel language shape implementation {duplicate}"
            );
        }
    }
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/group_source.rs")).unwrap();
    let header = source
        .split_once("pub(crate) fn header")
        .unwrap()
        .1
        .split_once("pub(crate) fn cold_headers")
        .unwrap()
        .0;
    assert!(header.contains("!valid_argument_name(&argument.name)"));
    assert!(!header.contains("preflight_native_group_members("));
    assert!(!header.contains("valid_member_name("));
    let canonical =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    let canonical_shape = canonical
        .split_once("pub(super) fn validate_shape")
        .unwrap()
        .1
        .split_once("pub(super) fn source")
        .unwrap()
        .0;
    assert!(canonical_shape.contains("branch.name.len() > MAX_BRANCH_NAME_BYTES"));
    assert!(!canonical_shape.contains("preflight_native_group_members("));
    for (path, markers) in [
        (
            "crates/leselang-hir-core/src/group_exports.rs",
            &["closed_export_shape_reads_each_original_label_once_without_native_traits"][..],
        ),
        (
            "crates/leselang-hir/tests/group_typing.rs",
            &[
                "shared_group_shape_preserves_exact_indices_and_precedes_atomic_child_preparation",
                "shared_group_shape_keeps_ir_label_grammar_and_current_inclusive_branch_limits",
            ][..],
        ),
        (
            "crates/leselang-hir/tests/group_exports.rs",
            &[
                "shared_group_shape_accepts_original_ir_labels_and_rechecks_current_export_ceiling",
                "shared_group_shape_rejects_complete_bad_names_before_tail_or_native_comparison",
            ][..],
        ),
        (
            "crates/leselang-hir/tests/effect_evaluation.rs",
            &[
                "shared_group_shape_rejects_bad_tail_and_arity_before_host_fuel_or_scope_changes",
                "shared_group_shape_keeps_ir_labels_and_inclusive_ceiling_before_native_policy",
            ][..],
        ),
        (
            "crates/leselang-hir/tests/group_source.rs",
            &["source_group_labels_keep_stricter_lexical_grammar_than_ir_member_names"][..],
        ),
    ] {
        let proof = std::fs::read_to_string(root.join(path)).unwrap();
        for marker in markers {
            assert!(
                proof.contains(marker),
                "missing shared group shape proof {marker}"
            );
        }
    }
}

#[test]
fn fixed_source_signatures_share_allocation_free_original_borrows_without_native_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/source_call.rs")).unwrap();
    let decoder = source
        .split_once("pub(crate) fn fixed_named_operands")
        .unwrap()
        .1
        .split_once("pub(crate) fn valid_limits")
        .unwrap()
        .0;
    for required in [
        "Option<[&'source Expression; N]>",
        "arguments.len() != N",
        "arguments.first()?.value",
        "expected.iter().take(index)",
        "matches.next()?.value",
        "matches.next().is_some()",
        "Some(ordered)",
    ] {
        assert!(
            decoder.contains(required),
            "fixed signature lost {required}"
        );
    }
    for forbidden in [
        "Vec",
        ".collect(",
        ".clone()",
        "Box",
        "unsafe",
        ".unwrap(",
        ".expect(",
        "panic!",
        "leserpent",
        "serde",
        "Fn",
        "Type:",
        "dispatch",
        "admit(",
    ] {
        assert!(
            !decoder.contains(forbidden),
            "metadata decoder gained {forbidden}"
        );
    }
    for (path, start, end) in [
        (
            "crates/leselang-hir-core/src/scalar_source.rs",
            "fn named",
            "pub(crate) fn primitive",
        ),
        (
            "crates/leselang-hir-core/src/choice_source.rs",
            "pub(crate) fn operands",
            "pub(crate) fn cold_names",
        ),
        (
            "crates/leselang-hir-core/src/projection_source.rs",
            "pub(crate) fn field_source",
            "pub(crate) struct MemberSource",
        ),
    ] {
        let caller = std::fs::read_to_string(root.join(path)).unwrap();
        let fixed = caller
            .split_once(start)
            .unwrap()
            .1
            .split_once(end)
            .unwrap()
            .0;
        assert!(fixed.contains("fixed_named_operands(arguments,"));
        for duplicate in ["Result<Vec", ".collect(", ".count()", ".find("] {
            assert!(
                !fixed.contains(duplicate),
                "duplicate fixed signature in {path}: {duplicate}"
            );
        }
    }
    let scalar =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/scalar_source.rs"))
            .unwrap();
    let cold = scalar
        .split_once("pub(crate) fn preflight_names")
        .unwrap()
        .1
        .split_once("struct Builder")
        .unwrap()
        .0;
    assert!(cold.contains("check_named::<Error, 1>"));
    assert!(cold.contains("check_named::<Error, 2>"));
    assert!(cold.contains("check_named::<Error, 3>"));
    let reference =
        std::fs::read_to_string(root.join("crates/leserpent-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("crate::choice_source::lower_choice_source("));
    assert!(reference.contains("field_source("));
    assert!(reference.contains("bind_scalar_form("));
    for (path, markers) in [
        (
            "crates/leselang-hir-core/src/source_call.rs",
            &[
                "fixed_signature_views_keep_original_operands_in_all_submission_orders",
                "fixed_signature_views_reject_missing_extra_duplicate_and_unknown_names",
                "fixed_signature_views_fail_closed_for_empty_or_duplicate_protocols",
            ][..],
        ),
        (
            "crates/leselang-hir/tests/scalar_source.rs",
            &[
                "fixed_scalar_operands_borrow_original_ast_in_semantic_order",
                "fixed_scalar_signatures_preserve_exact_diagnostics_before_native_work",
            ][..],
        ),
        (
            "crates/leselang-hir/tests/choice_source.rs",
            &[
                "shared_fixed_choice_signature_keeps_all_six_orders_and_original_native_observations",
            ][..],
        ),
        (
            "crates/leselang-hir/tests/projection_source.rs",
            &["shared_fixed_projection_signature_keeps_exact_errors_before_native_queries"][..],
        ),
    ] {
        let tests = std::fs::read_to_string(root.join(path)).unwrap();
        for marker in markers {
            assert!(
                tests.contains(marker),
                "missing fixed signature proof {marker}"
            );
        }
    }
}

#[test]
fn exact_ir_child_cursors_keep_compact_original_borrows_and_physical_budget_fences() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ir = std::fs::read_to_string(root.join("crates/leselang-hir-core/src/ir.rs")).unwrap();
    let cursor = ir
        .split_once("struct ChildNodes")
        .unwrap()
        .1
        .split_once("/// Bridge a trusted inferred scalar type")
        .unwrap()
        .0;
    for required in [
        "node: &'node Computation",
        "front: usize",
        "back: usize",
        "fn nth(",
        "fn nth_back(",
        "fn count(",
        "fn last(",
        "self.front = self.back",
        "self.back - self.front",
        "ExactSizeIterator",
        "FusedIterator",
    ] {
        assert!(cursor.contains(required), "child cursor lost {required}");
    }
    for forbidden in [
        "Vec",
        "Box",
        ".collect(",
        ".clone()",
        "unsafe",
        ".unwrap(",
        ".expect(",
        "panic!",
        "serde",
        "leserpent",
        "Clone +",
        "Debug +",
        "Send +",
    ] {
        assert!(
            !cursor.contains(forbidden),
            "child cursor gained {forbidden}"
        );
    }
    let children = ir
        .split_once("pub fn children(")
        .unwrap()
        .1
        .split_once("/// Classify language nodes only")
        .unwrap()
        .0;
    assert!(children.contains("ExactSizeIterator"));
    assert!(children.contains("ChildNodes"));
    assert!(!children.contains(".chain("));
    assert!(!children.contains(".flatten("));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/binding_source.rs"))
            .unwrap();
    let walk = source
        .split_once("pub(crate) fn physical")
        .unwrap()
        .1
        .split_once("/// Lower exactly one source Bind")
        .unwrap()
        .0;
    for required in [
        "let mut frames = Vec::new()",
        "let mut queued = 0usize",
        "node.children().rev()",
        "budget.check_pending(queued, children.len())?",
        "depth.checked_add(1)",
        ".checked_add(children.len())",
        "children.len() != 0",
        "frames.push((children, child_depth))",
        "queued.checked_sub(1)",
        "siblings.len() == 0",
        "frames.pop()",
    ] {
        assert!(
            walk.contains(required),
            "physical cursor walk lost {required}"
        );
    }
    assert!(walk.find("budget.visit(").unwrap() < walk.find("node.children().rev()").unwrap());
    let fence = walk
        .find("budget.check_pending(queued, children.len())?")
        .unwrap();
    assert!(fence < walk.find("children.next()").unwrap());
    assert!(fence < walk.find("depth.checked_add(1)").unwrap());
    assert!(fence < walk.find("frames.push(").unwrap());
    for forbidden in [
        "let mut pending",
        "pending.push(",
        "pending.extend(",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "unsafe",
        "FnMut",
        "admit(",
        "dispatch",
    ] {
        assert!(
            !walk.contains(forbidden),
            "physical cursor walk gained {forbidden}"
        );
    }
    for marker in [
        "cursor_physical_walk_matches_legacy_counts_and_error_precedence_across_limits",
        "cursor_physical_walk_keeps_reverse_cold_order_and_whole_sibling_root_fences",
        "cursor_physical_walk_bounds_wide_nodes_without_clamping_or_consuming_original_ir",
        "cursor_physical_walk_returns_structured_depth_overflow_and_keeps_node_error_priority",
    ] {
        assert!(
            source.contains(marker),
            "missing physical cursor proof {marker}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/generic_ir.rs")).unwrap();
    for marker in [
        "every_child_layout_preserves_declaration_order_original_borrows_and_fused_reverse_walks",
        "exact_child_cursors_keep_opaque_slots_and_checked_skip_ranges_without_native_traits",
        "std::mem::size_of_val(&node.children())",
        "nth_back(usize::MAX)",
        "mixed.size_hint()",
    ] {
        assert!(proof.contains(marker), "missing exact child proof {marker}");
    }
}

#[test]
fn shared_purity_uses_compact_cold_child_cursors_without_native_bounds_or_hidden_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ir = std::fs::read_to_string(root.join("crates/leselang-hir-core/src/ir.rs")).unwrap();
    let walk = ir.split_once("pub fn is_pure(&self) -> bool").unwrap().1;
    for required in [
        "let mut frames = Vec::new()",
        "let mut expression = self",
        "Self::Host { .. } | Self::Call { .. } | Self::Group { .. }",
        "expression.children().rev()",
        "children.len() != 0",
        "frames.push(children)",
        "siblings.len() == 0",
        "frames.pop()",
        "return false",
        "return true",
    ] {
        assert!(walk.contains(required), "purity cursor lost {required}");
    }
    assert!(walk.find("return false").unwrap() < walk.find("expression.children().rev()").unwrap());
    assert!(walk.find("children.len() != 0").unwrap() < walk.find("frames.push(").unwrap());
    for forbidden in [
        "vec![",
        "pending.extend(",
        "pending.push(",
        ".collect(",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "unsafe",
        "panic!",
        "FnMut",
        "validate_structure",
        "authorize(",
        "dispatch",
        "max_nodes",
        "max_depth",
    ] {
        assert!(
            !walk.contains(forbidden),
            "purity cursor gained {forbidden}"
        );
    }
    for path in [
        "crates/leserpent-hir/src/computation.rs",
        "crates/leselang-hir-core/src/helper_body.rs",
        "crates/leselang-hir-core/src/group_exports.rs",
    ] {
        let consumer = std::fs::read_to_string(root.join(path)).unwrap();
        assert!(consumer.contains(".is_pure()"));
        assert!(!consumer.contains("fn is_pure("));
    }
    let bridge = ir
        .split_once("pub fn scalar_argument_type(")
        .unwrap()
        .1
        .split_once("/// Borrow direct language children")
        .unwrap()
        .0;
    assert!(bridge.contains("is_pure: self.is_pure()"));
    assert!(bridge.contains("Self::Literal { value } => Some(value)"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/generic_ir.rs")).unwrap();
    for marker in [
        "cursor_purity_matches_legacy_for_every_cold_control_position_without_type_admission",
        "cursor_purity_keeps_wide_and_deep_original_nodes_without_hidden_limits_or_stale_results",
        "cursor_purity_borrows_opaque_nonclone_slots_without_native_observation_or_consumption",
        "cursor_scalar_argument_bridge_keeps_borrowed_literals_and_untrusted_type_hints",
        "fn legacy_purity<",
        "std::ptr::eq(argument.literal.unwrap(), value)",
    ] {
        assert!(
            proof.contains(marker),
            "missing shared purity proof {marker}"
        );
    }
}

#[test]
fn catalog_expression_pipeline_owns_source_orchestration_before_native_metadata_and_uses_shared_sessions()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_source.rs"))
            .unwrap();
    let entry = source
        .split_once("pub fn compile_catalog_source")
        .unwrap()
        .1;
    assert!(
        entry.find("preflight_control_source(").unwrap() < entry.find("cold_catalog(").unwrap()
    );
    assert!(
        entry.find("cold_catalog(").unwrap()
            < entry.find("lower_preflighted_control_source(").unwrap()
    );
    for required in [
        "CatalogSourceEnvironment",
        "host.catalog",
        ".authorize(",
        ".bind_arguments(&names)",
        "lower_scalar_source_with_scope(",
        "lower_catalog_projection_source(",
        "lower_source_call(",
        "operation != schema.key",
        "operation.borrow() != schema.key.borrow()",
        "infer_call_flow_type(",
        "max_calls: self.limits.max_calls",
        "CatalogSourceError::InconsistentType",
    ] {
        assert!(
            source.contains(required),
            "catalog compiler lost {required}"
        );
    }
    let hooks = source
        .split_once("pub trait CatalogSourceEnvironment")
        .unwrap()
        .1
        .split_once("pub enum CatalogSourceError")
        .unwrap()
        .0;
    for forbidden in [
        "lower_leaf",
        "lower_child",
        "FnMut",
        "FnOnce",
        "Clone +",
        "serde",
        "Send +",
    ] {
        assert!(
            !hooks.contains(forbidden),
            "metadata adapter gained recursive/native requirement {forbidden}"
        );
    }
    for forbidden in [
        "leserpent_domain",
        "leselang_vm",
        "rusqlite",
        "unsafe",
        ".unwrap(",
        ".expect(",
        "panic!",
    ] {
        assert!(
            !source.contains(forbidden),
            "catalog compiler gained {forbidden}"
        );
    }
    let record =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/bound_projection_session.rs"))
            .unwrap();
    assert!(record.contains("compile_catalog_source("));
    assert!(!record.contains("impl<'source, 'schema> ControlSourceAdapter"));
    assert!(!record.contains("fn lower_leaf("));
    assert!(!record.contains("lower_source_call("));
    assert!(!record.contains("infer_call_flow_type("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_source.rs")).unwrap();
    for marker in [
        "one_shared_compiler_uses_unrelated_catalogs_with_nonclone_native_slots",
        "complete_cold_catalog_versions_grants_and_names_precede_every_source_hook",
        "whole_physical_lexical_and_intrinsic_signatures_stop_native_mapping",
        "scalar_bind_loop_fold_and_pure_tail_have_one_catalog_compilation_entry",
        "mapped_operation_identity_failure_unwind_and_type_rejection_never_return_partial_ir",
        "compiled_scalar_catalog_program_suspends_accepts_typed_reply_reenters_and_cancels_without_product_vm",
        "stale.reply",
        "cancelled.cancel()",
        "Rc::strong_count(&env.owner)",
    ] {
        assert!(
            proof.contains(marker),
            "missing catalog source vertical proof {marker}"
        );
    }
}

#[test]
fn catalog_multifunction_entry_owns_helper_recursion_hygiene_and_nonrefunding_work_bounds() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let program =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_program.rs"))
            .unwrap();
    let entry = program
        .split_once("pub fn compile_catalog_program")
        .unwrap()
        .1;
    for preflight in [
        "accept_helper_declarations(",
        "helper_dependency_order(",
        "preflight_control_source(",
        "cold_catalog(",
        "bind_named_arguments(",
    ] {
        assert!(entry.find(preflight).unwrap() < entry.find("program.body(").unwrap());
    }
    for shared in [
        "lower_catalog_expression(",
        "lower_helper_arguments(",
        "hygienic_helper_body(",
        "bind_helper_arguments(",
        "prepared.nodes",
        "self.charge(prepared.parameters.len())?",
        "max_compilation_nodes",
        "max_expansion_depth",
        "checked_add(nodes)",
        "struct Depth",
        "impl Drop for Depth",
        "CatalogProgramError::CompilationLimit",
        "CatalogProgramError::ExpansionDepth",
        "host.catalog.lookup(name, host.catalog.version())",
        "!valid_local_name(name)",
    ] {
        assert!(
            program.contains(shared),
            "missing shared helper boundary {shared}"
        );
    }
    for forbidden in [
        "leserpent_domain",
        "leselang_vm",
        "rusqlite",
        "unsafe",
        ".unwrap(",
        ".expect(",
        "panic!",
        "Clone +",
        "Send +",
        "Serialize +",
    ] {
        assert!(
            !program.contains(forbidden),
            "standalone helper compiler gained {forbidden}"
        );
    }
    let compiler =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_source.rs"))
            .unwrap();
    assert!(compiler.contains("program.lower_helper("));
    assert!(compiler.contains("let scalar_types: Vec<_> = local"));
    assert!(compiler.contains(".bindings()"));
    assert!(compiler.contains("infer_call_flow_type("));
    let adapter = compiler
        .split_once("pub trait CatalogSourceEnvironment")
        .unwrap()
        .1
        .split_once("pub enum CatalogSourceError")
        .unwrap()
        .0;
    assert!(!adapter.contains("lower_helper"));
    assert!(!adapter.contains("FnMut"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_source.rs")).unwrap();
    for marker in [
        "shared_multifunction_compiler_owns_forward_calls_scalar_parameters_and_explicit_entry",
        "whole_unused_helper_policy_names_and_cycles_precede_every_native_compilation_hook",
        "helper_hygiene_preserves_caller_aliases_and_nested_scalar_loop_fold_scopes",
        "effectful_helper_compilation_reenters_shared_session_and_cancels_without_native_ir_copy",
        "helper_recompilation_is_bounded_before_exponential_source_work_or_partial_output",
        "multifunction_catalog_hosts_remain_unrelated_and_failure_unwind_releases_native_slots",
        "compile_catalog_program(",
        "cancelled.cancel()",
    ] {
        assert!(
            proof.contains(marker),
            "missing multi-function vertical {marker}"
        );
    }
    let record =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/bound_projection_session.rs"))
            .unwrap();
    assert!(record.contains(
        "multifunction_record_host_preserves_original_reply_fields_live_policy_and_cancel_ownership"
    ));
    assert!(record.contains("compile_catalog_program("));
    assert!(record.contains("revoked.poll(host).is_err()"));
}

#[test]
fn group_entry_program_reuses_helper_preflight_hygiene_work_and_final_flow_without_native_compiler_hooks()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let group =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_group_source.rs"))
            .unwrap();
    let program =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_program.rs"))
            .unwrap();
    let entry = group
        .split_once("pub fn compile_catalog_group_program")
        .unwrap()
        .1;
    for required in [
        "compile_catalog_program_with_entry(",
        "program.with_expansion(",
        "lower_group_catalog_expression(",
        "Some(program)",
        "CatalogSourceError::Group",
    ] {
        assert!(entry.contains(required), "group entry lost {required}");
    }
    assert!(group.contains(".with_program(program)"));
    assert!(group.contains("is_helper(&member.value)"));
    let shared = program
        .split_once("pub(crate) fn compile_catalog_program_with_entry")
        .unwrap()
        .1;
    for preflight in [
        "accept_helper_declarations(",
        "helper_dependency_order(",
        "preflight_closed_control_source(",
        "cold_groups_with(",
        "bind_named_arguments(",
        "cold_catalog_profile(",
    ] {
        assert!(shared.find(preflight).unwrap() < shared.find("program.body(").unwrap());
    }
    let entry_tail = shared.split_once("program.charge(entry_nodes)?").unwrap().1;
    assert!(entry_tail.contains("finish(&program,"));
    assert!(program.contains("let _depth = Depth(&self.depth, previous)"));
    assert!(program.contains("profile.helper_groups || std::ptr::eq(*function, accepted.entry())"));
    assert!(entry.contains("helper_groups: false"));
    let original = program
        .split_once("pub fn compile_catalog_program")
        .unwrap()
        .1
        .split_once("pub(crate) fn compile_catalog_program_with_entry")
        .unwrap()
        .0;
    assert!(original.contains("None,"));
    assert!(original.contains("program.compile(source, prefix, host, environment)"));
    for forbidden in [
        "hygienic_helper_body(",
        "bind_helper_arguments(",
        "fn parse",
        "source.clone()",
        "body.clone()",
    ] {
        assert!(
            !group.contains(forbidden),
            "group entry duplicated {forbidden}"
        );
    }
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for proof in [
        "group_program_entry_uses_forward_pure_helpers_without_native_slot_copies",
        "group_program_entry_combines_ordinary_effect_helpers_and_closed_result_projections",
        "group_program_cold_unused_helpers_references_and_dependency_policy_precede_native_mapping",
        "group_program_helpers_cannot_masquerade_as_atomic_members_or_return_groups",
        "group_program_preserves_shared_nonrefunding_work_depth_and_whole_output_limits",
        "group_program_branch_failure_and_unwind_release_expanded_native_output_once",
        "legacy_program_remains_group_rejecting_and_new_entry_needs_no_groups_for_pure_helpers",
    ] {
        assert!(tests.contains(proof));
    }
    let vertical =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_reply_reentry.rs"))
            .unwrap();
    assert!(vertical.contains(
        "parsed_group_entry_program_reenters_hygienic_helper_tail_with_original_reply_and_fuel"
    ));
    assert!(vertical.contains("helper_tail_never_runs_after_revocation_or_outer_cancellation"));
    assert!(vertical.contains("fuel.remaining(), 73"));
}

#[test]
fn group_helper_program_keeps_language_owned_lowering_and_original_native_reentry_protocol() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let group =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_group_source.rs"))
            .unwrap();
    let program =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_program.rs"))
            .unwrap();
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_source.rs"))
            .unwrap();
    let entry = group
        .split_once("pub fn compile_catalog_group_helper_program")
        .unwrap()
        .1;
    for required in [
        "compile_catalog_program_with_entry(",
        "helper_groups: true",
        "HelperGroups {",
        ".compile(source, prefix)",
    ] {
        assert!(entry.contains(required));
    }
    for required in [
        "fn body_with<",
        "fn lower_helper_with<",
        "self.body_with(prepared, &lower)",
        "self.with_expansion(|| lower(&argument.value, prefix))",
        "cold_groups = cold_groups",
        ".checked_add(count)",
    ] {
        assert!(
            program.contains(required),
            "shared helper compiler lost {required}"
        );
    }
    assert!(source.contains("pub(crate) trait CatalogLocalSource"));
    assert!(source.contains("self.locals.lower(source, scope.bindings())"));
    assert!(group.contains(".lower_helper_with(source, prefix,"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for required in [
        "group_returning_forward_helpers_move_original_slots_through_shared_hygiene",
        "parameterized_group_helper_prepares_original_operands_before_parameter_scope_is_dropped",
        "whole_unused_group_helper_headers_references_and_catalog_policy_precede_native_hooks",
        "cold_group_declaration_forest_and_expanded_helper_groups_have_distinct_total_gates",
        "group_return_helper_wrappers_keep_inclusive_work_depth_and_whole_output_quotas",
        "group_return_helpers_cannot_be_scalar_operands_or_union_incompatible_exports",
        "group_helper_native_failure_or_unwind_drops_unused_validation_without_retry_or_copies",
        "group_helper_profile_is_opt_in_and_entry_only_or_call_only_boundaries_are_unchanged",
        "group_helper_compilation_does_not_silently_enable_nested_suspension_scheduling",
    ] {
        assert!(proof.contains(required));
    }
    let vertical =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_reply_reentry.rs"))
            .unwrap();
    assert!(vertical.contains(
        "parsed_group_return_helper_reenters_original_control_and_hygienic_tail_with_one_meter"
    ));
    assert!(
        vertical
            .contains("group_return_helper_does_not_bypass_outer_reply_revocation_or_cancellation")
    );
}

#[test]
fn prepared_source_member_profile_reuses_cold_helper_facts_and_original_uniform_inference() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let group =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_group_source.rs"))
            .unwrap();
    let old = group
        .split_once("pub fn compile_catalog_group_helper_program")
        .unwrap()
        .1
        .split("/// Opt in to pure Bind/Choose")
        .next()
        .unwrap();
    assert!(old.contains("environment, false)"));
    let new = group
        .split_once("pub fn compile_catalog_prepared_group_helper_program")
        .unwrap()
        .1
        .split("fn compile_group_helpers<")
        .next()
        .unwrap();
    assert!(new.contains("environment, true)"));
    assert_eq!(group.matches("fn compile_group_helpers<").count(), 1);
    assert!(group.contains("let prepared_members = locals.prepared_members()"));
    assert!(group.contains("lower_preflighted_control_source("));
    assert!(
        group
            .find("let (operation, result) = infer_prepared_call_observation(")
            .unwrap()
            < group.find(".source_group_branch(original,").unwrap()
    );
    assert!(group.contains("!std::ptr::eq(result, &schema.result)"));
    let program =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_program.rs"))
            .unwrap();
    assert!(
        program.find("cold_prepared_members(").unwrap()
            < program.find("for function in order {").unwrap()
    );
    let facts =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_atomic_source.rs"))
            .unwrap();
    for required in [
        "binding_source::<()>",
        "operands::<()>",
        "for helper in ordered_helpers",
        "for function in declarations",
        "Shape::Atomic(_)",
        "sequential_exports(node, max_branches)?",
        "joined_member_name_fits(&longest, name)",
        "joined_member_name_fits(&argument.name, &name)",
    ] {
        assert!(facts.contains(required));
    }
    assert!(
        facts.find(".checked_mul(count)").unwrap()
            < facts.find("Vec::with_capacity(width)").unwrap()
    );
    let exports =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/group_exports.rs"))
            .unwrap();
    let join = exports
        .split_once("fn joined_member_name_fits")
        .unwrap()
        .1
        .split_once("#[derive")
        .unwrap()
        .0;
    for required in [
        "valid_argument_name(prefix)",
        "valid_argument_name(member)",
        "checked_add(2)",
        "checked_add(member.len())",
        "length <= MAX_LOCAL_NAME_BYTES",
    ] {
        assert!(
            join.contains(required),
            "shared export name bound lost {required}"
        );
    }
    for forbidden in [
        "leserpent",
        "leselang_vm",
        "rusqlite",
        "serde::",
        "evaluate_",
        "Fuel::new",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "panic!",
        "unsafe ",
    ] {
        assert!(
            !facts.contains(forbidden),
            "cold shape facts gained {forbidden}"
        );
    }
    let typing =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/prepared_typing.rs"))
            .unwrap();
    assert!(typing.contains(
        "infer_prepared_call_observation(expression, bindings, environment, schemas, limits)"
    ));
    assert!(typing.contains("Ok((original_operation, result))"));
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for proof in [
        "prepared_member_profile_is_explicit_and_preserves_legacy_direct_member_rejection",
        "prepared_members_reject_hidden_effect_shapes_in_unused_helpers_before_native_mapping",
        "prepared_member_named_catalog_dependency_and_closed_reference_policy_remain_cold",
        "prepared_member_inline_bind_choose_and_forward_aliases_use_original_uniform_rows",
        "prepared_member_expansion_keeps_shared_work_depth_group_and_shifted_output_limits",
        "prepared_member_native_failure_and_unwind_release_moved_output_without_retries",
        "composed_export_faults_in_any_declaration_precede_all_native_hooks",
        "cold_composed_export_limits_match_materialized_width_order_and_exact_name_boundaries",
        "cold_composed_export_diagnostics_preserve_source_positions_without_native_payloads",
        "parsed_atomic_helper_members_use_seq_all_original_replies_and_one_enclosing_meter",
        "parsed_atomic_helper_members_preparation_refusal_or_fuel_exhaustion_never_dispatches",
        "parsed_atomic_helper_members_cancel_or_revocation_stops_next_publication_and_caller",
        "assert_eq!(session.fuel_remaining(), 260)",
        "assert_eq!(session.fuel_remaining(), 244)",
    ] {
        assert!(proofs.contains(proof), "prepared source proof lost {proof}");
    }
}

#[test]
fn nested_return_frames_reuse_one_walker_and_original_reply_protocol_without_a_second_runtime() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_stack.rs")).unwrap();
    let production = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "panic!",
        "match expression",
        "evaluate_preflighted_in_scope",
    ] {
        assert!(
            !production.contains(forbidden),
            "return frames gained {forbidden}"
        );
    }
    for required in [
        "pub struct EffectReturnStack",
        "pub struct SavedEffectReturnFrame",
        "pub fn restore_effect_return_stack<",
        "pub struct EffectStackLimits",
        "Declaration: ?Sized",
        "evaluate_with_profile::<",
        "preflight_into(",
        "restore_effect_scope_with_liveness(",
        "bind_effect_value(",
        "continuation.frames.extend(frames.by_ref())",
        ".checked_sub(frames.len())",
        "EffectStackFault::ReturnLimit",
    ] {
        assert!(source.contains(required), "return frames lost {required}");
    }
    let resume = source
        .split_once("pub fn resume_accepted_stacked_effects<")
        .unwrap()
        .1;
    assert!(
        resume.find("preflight_into(").unwrap() < resume.find(".preflight_effect(effect)").unwrap()
    );
    assert!(
        resume.find(".preflight_effect(effect)").unwrap()
            < resume.find("restore_effect_scope_with_liveness(").unwrap()
    );
    assert_eq!(resume.matches(".bind_reply(").count(), 1);
    let walker =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_evaluation.rs"))
            .unwrap();
    assert!(walker.contains("pub(crate) trait SuspensionProfile"));
    assert!(walker.contains("evaluate_with_profile::<_, _, _, _, _, SingleFrame>"));
    assert!(
        walker.find("Profile::admits(Some(&continuation)").unwrap()
            < walker.rfind(".capture(name, body, scope, fuel)").unwrap()
    );
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_stack.rs")).unwrap();
    for proof in [
        "nested_helpers_return_through_original_scopes_and_repeated_requests_without_fuel_reset",
        "saved_stack_admission_moves_native_captures_and_reuses_the_original_reply_walker",
        "saved_stack_refuses_empty_names_and_shared_forest_overflow_without_native_work",
        "saved_stack_admission_neither_grants_live_authority_nor_rearms_cancelled_owners",
        "request_tail_adopts_the_next_original_return_site_without_replaying_inner_effects",
        "legacy_single_frame_entry_still_rejects_nested_suspension_before_outer_capture",
        "return_quota_is_inclusive_shared_across_new_and_pending_frames_and_never_refunds",
        "newly_suspended_helper_frames_cannot_reset_the_pending_outer_return_quota",
        "invalid_limits_and_cold_whole_return_forest_denial_precede_restore_projection_and_fuel",
        "cold_denial_in_a_later_outer_body_precedes_every_inner_restore_and_projection",
        "wrong_identity_preserves_original_input_and_cancellation_discards_all_return_frames",
        "live_revocation_after_acceptance_and_cancel_cleanup_unwind_cannot_rearm_frames",
        "outer_capture_failure_or_unwind_releases_inner_frames_and_prepared_request_once",
        "restoration_and_projection_failures_unwind_or_bad_outer_prefix_never_replay_dispatch",
        "zero_frame_pure_and_final_request_paths_remain_available_without_native_debug_or_clone_bounds",
    ] {
        assert!(tests.contains(proof));
    }
    let group =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_reply_reentry.rs"))
            .unwrap();
    for proof in [
        "parsed_helper_internal_group_reply_unwinds_to_original_outer_return_without_new_vm_or_dispatch",
        "nested_group_helper_return_frames_stop_on_cancel_or_fresh_reentry_revocation",
        "parsed_native_group_value_returns_through_outer_frame_without_a_second_reply_projection",
    ] {
        assert!(group.contains(proof));
    }
}

#[test]
fn prepared_sequence_reuses_original_reply_core_with_complete_correlation_and_fresh_single_publication()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/prepared_sequence.rs"))
            .unwrap();
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/sequence_evaluation.rs"))
            .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    let production = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "Fuel::new",
        ".clone()",
        "serde::",
        "leselang_vm",
        "leserpent_domain",
        "rusqlite",
        "panic!",
        ".unwrap(",
        ".expect(",
        "unsafe",
        "Identity: Clone",
        "Request: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "prepared sequence gained {forbidden}"
        );
    }
    let fields = source
        .split_once("pub struct PreparedSequenceSession<")
        .unwrap()
        .1
        .split_once("impl<")
        .unwrap()
        .0;
    assert!(fields.contains("core: SequenceReplyCore<"));
    assert!(fields.contains("prepared: VecDeque<PreparedSequenceMember<"));
    assert!(!fields.contains("fuel:"));
    assert!(!fields.contains("state:"));
    assert_eq!(shared.matches("enum State<").count(), 1);
    assert_eq!(shared.matches("PendingReply::new(").count(), 2);
    assert!(shared.contains("core: SequenceReplyCore<"));
    for boundary in [
        ".publish_with(",
        "self.core.try_accept_with_scope(identity,reply,authority,scope)",
        "self.core.cancel()",
        "std::mem::take(&mutself.prepared)",
    ] {
        assert!(
            compact.contains(boundary),
            "shared sequence boundary lost {boundary}"
        );
    }
    let ordered = [
        "admit_sequence_shape(group,limits)",
        "std::ptr::eq(prepared.metadata().members(),branches)",
        "row.index()!=index",
        "admit(index,row.original(),row.prepared())",
        "fuel.charge(1)",
        "correlate(index,original,operand,fuel)",
        "ifmember.identity==earlier.identity",
        "core:SequenceReplyCore::new(branches)",
    ];
    for pair in ordered.windows(2) {
        assert!(compact.find(pair[0]).unwrap() < compact.find(pair[1]).unwrap());
    }
    let poll = compact
        .split_once("pubfnpoll<Error>")
        .unwrap()
        .1
        .split_once("pubfntry_accept<")
        .unwrap()
        .0;
    assert!(
        poll.find("admit(index,original,").unwrap() < poll.find("prepared.pop_front()").unwrap()
    );
    assert!(!poll.contains("fuel.charge"));
    assert!(!poll.contains("correlate("));
    let accept = compact
        .split_once("pubfntry_accept<")
        .unwrap()
        .1
        .split_once("pubfncancel")
        .unwrap()
        .0;
    assert!(!accept.contains("publish_with"));
    assert!(!accept.contains("admit("));
    let cancel = compact.split_once("pubfncancel").unwrap().1;
    assert!(cancel.find("std::mem::take").unwrap() < cancel.find("self.core.cancel()").unwrap());
    assert!(cancel.find("self.core.cancel()").unwrap() < cancel.find("drop(prepared)").unwrap());
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_sequence.rs"))
            .unwrap();
    for proof in [
        "complete_scoped_correlation_then_one_request_per_accepted_reply_uses_original_rows_and_one_meter",
        "shape_foreign_original_rows_and_all_prepared_admission_precede_correlation_and_additional_fuel",
        "late_correlation_duplicate_identity_and_exhaustion_return_no_session_and_drop_every_operand",
        "admission_correlation_identity_or_prefix_cleanup_unwind_never_publishes_a_request",
        "publication_refusal_revocation_or_unwind_closes_before_queued_suffix_cleanup",
        "rejected_identity_authority_type_and_value_preserve_owned_input_and_all_queued_work",
        "acceptance_hook_unwind_or_cancel_identity_cleanup_seals_and_releases_all_future_requests",
        "inclusive_single_and_maximum_width_need_no_poll_meter_and_cancel_unpublished_work",
        "unsized_original_domain_and_borrowed_text_view_preserve_rejection_buffer",
    ] {
        assert!(tests.contains(proof));
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for proof in [
        "parsed_parameterized_seq_helper_publishes_only_after_each_reply_then_restores_one_root_meter",
        "parsed_prepared_seq_cancel_or_fresh_revocation_never_invokes_the_second_member_or_caller",
        "ParamSequence::start(",
        "sequence.poll(",
        "self.admit_prepared(original, request)?",
        "assert_eq!(session.fuel_remaining(), 78)",
        "assert_eq!(session.fuel_remaining(), 62)",
    ] {
        assert!(parsed.contains(proof));
    }
}

#[test]
fn stacked_control_sessions_delegate_owned_fuel_and_all_lifecycle_transitions_to_the_original_core()
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_stack_session.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "panic!",
        "enum State",
        "PendingReply::new",
        "Fuel::new",
        "state:",
        "fuel: Fuel,\n    limits:",
    ] {
        assert!(
            !production.contains(forbidden),
            "stacked session gained {forbidden}"
        );
    }
    for required in [
        "pub struct StackedControlSession",
        "pub type StackedEffectSession",
        "Declaration: ?Sized",
        "SessionCore::new(expression, bindings, fuel)",
        "self.core.cancel()",
        "self.core.try_accept(identity, reply, authority)",
        "self.core.poll_with(",
        "evaluate_stacked_effects_in_scope(",
        "resume_accepted_stacked_effects_with_scope(",
        "stack_failure",
    ] {
        assert!(source.contains(required), "stacked session lost {required}");
    }
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_session.rs"))
            .unwrap();
    assert_eq!(shared.matches("enum State<").count(), 1);
    assert_eq!(shared.matches("fn poll_with<").count(), 1);
    let active_core = shared
        .split("pub(crate) struct SessionCore<")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    assert_eq!(active_core.matches("fuel: Fuel").count(), 1);
    let transfer = shared
        .split("pub(crate) fn try_into_pending_binding(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn cancel(")
        .next()
        .unwrap();
    assert!(transfer.contains("let Self { state, fuel } = self;"));
    assert!(transfer.contains("Ok(PendingSessionParts {"));
    assert!(transfer.contains("state => Err(Self { state, fuel })"));
    assert!(!transfer.contains("Fuel::new"));
    assert!(!transfer.contains(".clone()"));
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "stacked_session_owns_repeated_request_returns_original_scopes_and_one_meter",
        "stacked_session_preserves_original_rejected_input_and_acceptance_is_not_execution",
        "stacked_session_cancel_initial_pending_accepted_or_final_keeps_all_later_polls_inert",
        "stacked_session_drop_releases_every_pending_or_accepted_frame_without_reentry",
        "stacked_session_limits_are_terminal_and_legacy_session_still_rejects_nested_returns",
        "stacked_session_every_acceptance_unwind_seals_before_all_frame_cleanup",
        "stacked_session_preparation_correlation_restore_and_projection_faults_never_rearm",
        "stacked_session_cleanup_unwind_keeps_cancelled_or_host_uncertain_without_partial_handoff",
        "stacked_session_revocation_and_exhaustion_after_acceptance_do_not_refill_or_replay",
        "stacked_session_zero_frames_pure_final_reply_bounds_and_debug_remain_native_free",
    ] {
        assert!(proofs.contains(proof));
    }
    let group =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_reply_reentry.rs"))
            .unwrap();
    for proof in [
        "parsed_group_helpers_use_owned_fuel_stacked_sessions_with_explicit_native_work_reservation",
        "stacked_group_session_cancel_or_revocation_after_acceptance_never_projects_or_restores",
        "stacked_group_session_exhausted_native_reservation_cannot_publish_partial_work",
    ] {
        assert!(group.contains(proof));
    }
}

#[test]
fn complete_native_group_preparation_reuses_cold_admission_and_one_caller_meter_without_partial_handoff()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("crates/leselang-hir-core/src/native_group_preparation.rs"),
    )
    .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "panic!",
        "unsafe ",
        "Fuel::new",
        "ScopeFrame::new",
        "StructureBudget::new",
        "inspect_native_graph(",
        "evaluate_call_arguments_in_scope(",
    ] {
        assert!(
            !production.contains(forbidden),
            "native group preparation gained {forbidden}"
        );
    }
    for required in [
        "pub struct PreparedNativeMember",
        "pub struct PreparedNativeGroup",
        "pub enum NativeGroupPreparationError",
        "pub fn prepare_native_group_members",
        "fuel: &mut Fuel",
        "impl FnMut(usize, &'group Branch, &mut Fuel)",
        "Vec::with_capacity(members.len())",
        "member_index: Some(member_index)",
        "let value = prepare(member_index, original, fuel)",
        "original,",
        "prepared: value",
    ] {
        assert!(production.contains(required), "preparation lost {required}");
    }
    let entry = production
        .split_once("pub fn prepare_native_group_members")
        .unwrap()
        .1;
    assert!(
        entry.find("admit_native_group_members(").unwrap() < entry.find("fuel.charge(1)").unwrap()
    );
    assert!(entry.find("fuel.charge(1)").unwrap() < entry.find("Vec::with_capacity").unwrap());
    assert!(entry.rfind("fuel.charge(1)").unwrap() < entry.find("let value = prepare(").unwrap());
    assert!(entry.find("prepared.push(").unwrap() < entry.find("Ok(PreparedNativeGroup").unwrap());
    assert_eq!(
        entry
            .matches("prepare(member_index, original, fuel)")
            .count(),
        1
    );
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_preparation.rs"))
            .unwrap();
    for proof in [
        "original_member_slots_and_owned_preparations_are_linked_once_without_native_trait_bounds",
        "complete_cold_names_and_native_admission_precede_every_prepare_and_fuel_debit",
        "root_and_each_member_charge_share_one_original_meter_and_drop_exhausted_prefixes",
        "late_native_failure_or_unwind_drops_owned_prefix_without_partial_handoff_retry_or_refund",
        "prefix_cleanup_unwind_still_releases_all_other_preparations_without_returning_a_batch",
        "sequence_parallel_and_inclusive_sixty_four_member_ceiling_use_the_shared_original_name_gate",
    ] {
        assert!(proofs.contains(proof));
    }
    let group =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for proof in [
        "parsed_parameterized_group_helper_prepares_live_operands_and_restores_both_scopes_through_owned_session",
        "parameterized_group_session_cancellation_revocation_or_late_preparation_refusal_stops_successors",
        "prepare_native_group_members(",
        "evaluate_call_arguments_in_scope(",
        "assert_eq!(waiting_fuel, 81)",
        "assert_eq!(session.fuel_remaining(), 68)",
    ] {
        assert!(group.contains(proof));
    }
}

#[test]
fn prepared_parallel_sessions_reuse_original_reply_core_without_argument_replay_or_a_second_meter()
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/prepared_parallel.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "enum State",
        "PendingReply::new",
        "Fuel::new",
        "fuel: Fuel",
        "evaluate_pure",
        "evaluate_call_arguments",
        "ScopeFrame::new",
        "leserpent",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "panic!",
        "unsafe ",
    ] {
        assert!(
            !production.contains(forbidden),
            "prepared barrier gained {forbidden}"
        );
    }
    for required in [
        "pub struct PreparedParallelSession",
        "PreparedNativeGroup",
        "admit_parallel_shape(group, limits)",
        "prepared.metadata().members()",
        "row.index() != index",
        "admit(index, row.original(), row.prepared())",
        "fuel: &mut Fuel",
        "ParallelReplyCore::new(branches)",
        ".publish_with(",
        "row.into_parts()",
        ".poll_results(None)",
        "self.core.try_accept(index, identity, reply, authority)",
        "self.core.cancel()",
        "Declaration: ?Sized",
    ] {
        assert!(
            source.contains(required),
            "prepared barrier lost {required}"
        );
    }
    assert!(
        source.find("admit_parallel_shape(group, limits)").unwrap()
            < source.find("admit(index,").unwrap()
    );
    assert!(source.find("admit(index,").unwrap() < source.find("fuel.charge(1)").unwrap());
    assert!(source.find("fuel.charge(1)").unwrap() < source.find("correlate(index,").unwrap());
    assert!(
        source
            .split_whitespace()
            .collect::<String>()
            .contains(".publish_with(fuel,")
    );
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/parallel_evaluation.rs"))
            .unwrap();
    assert_eq!(shared.matches("enum State<").count(), 1);
    assert_eq!(shared.matches("enum Member<").count(), 1);
    assert_eq!(shared.matches("PendingReply::new(").count(), 2);
    assert_eq!(shared.matches("pub(crate) fn try_accept<").count(), 1);
    assert!(
        shared
            .split_whitespace()
            .collect::<String>()
            .contains("self.core.publish_with(&mutself.fuel,")
    );
    assert!(shared.contains("self.core.try_accept(index, identity, reply, authority)"));
    assert!(shared.contains("if !matches!(self.state, State::Ready)"));
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_parallel.rs"))
            .unwrap();
    for proof in [
        "complete_scoped_operands_move_once_to_the_original_out_of_order_reply_barrier_without_a_new_meter",
        "foreign_prepared_rows_shape_and_all_native_admission_fail_before_correlation_or_additional_fuel",
        "correlation_refusal_duplicate_identity_and_exhaustion_consume_all_preparations_without_partial_handoff",
        "native_admission_correlation_identity_and_prefix_cleanup_unwind_publish_nothing_and_drop_suffixes",
        "wrong_member_identity_domain_or_authority_keeps_original_input_and_never_exposes_partial_results",
        "acceptance_unwind_and_cancel_cleanup_use_the_same_terminal_fencing_as_the_legacy_barrier",
        "inclusive_maximum_width_preserves_the_original_meter_and_ready_results_cancel_without_handoff",
        "unsized_original_domains_and_borrowed_reply_views_work_without_payload_or_ir_clone_bounds",
    ] {
        assert!(proofs.contains(proof));
    }
    let group =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for proof in [
        "parsed_parameterized_helper_uses_prepared_original_member_barrier_and_one_enclosing_meter",
        "prepared_helper_barrier_cancel_revocation_or_late_correlation_denial_never_reenters_caller",
        "ParamBarrier::start(",
        "assemble_group_reply(",
        "assert_eq!(session.fuel_remaining(), 78)",
        "assert_eq!(session.fuel_remaining(), 62)",
    ] {
        assert!(group.contains(proof));
    }
}

#[test]
fn prepared_host_expression_entry_reuses_program_gates_without_synthetic_helpers() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_group_source.rs"))
            .unwrap();
    let entry = source
        .split_once("pub fn compile_catalog_prepared_group_source")
        .unwrap()
        .1
        .split_once("fn lower_group_catalog_expression")
        .unwrap()
        .0;
    for pair in [
        "preflight_closed_control_source(source, prefix,",
        "cold_groups_with(source, limits, true,",
        "cold_catalog_with_repeat(source, host,",
        "cold_prepared_source(source, limits.max_branches)",
        "lower_group_catalog_expression(",
    ]
    .windows(2)
    {
        assert!(entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap());
    }
    for forbidden in [
        "compile_catalog_program",
        "Function {",
        "parse(",
        "Fuel::new",
        "evaluate_",
        "serde",
        "unsafe ",
    ] {
        assert!(
            !entry.contains(forbidden),
            "expression entry gained {forbidden}"
        );
    }
    assert!(entry.contains("PreparedExpression"));
    let cold =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/catalog_atomic_source.rs"))
            .unwrap();
    assert!(cold.contains("cold_members(&function.body, ordered_helpers, &facts, max_branches)"));
    assert!(cold.contains("cold_members(source, &[], &[], max_branches)"));
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/catalog_group_source.rs"))
            .unwrap();
    for proof in [
        "prepared_expression_uses_borrowed_prefix_without_helpers_or_legacy_profile_widening",
        "prepared_expression_rejects_invalid_prefix_catalog_and_member_exports_before_mapping",
        "prepared_expression_preserves_original_native_result_prefix_types",
        "prepared_expression_preserves_output_bounds_and_move_only_cleanup_on_native_refusal",
    ] {
        assert!(proofs.contains(proof), "missing expression proof {proof}");
    }
}

#[test]
fn unrelated_gui_prepared_source_uses_shared_compiler_sessions_and_original_native_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_gui_host.rs"))
            .unwrap();
    for forbidden in [
        "leselang_vm",
        "leserpent_protocol",
        "rusqlite",
        "serde_json::",
        "unsafe ",
    ] {
        assert!(!source.contains(forbidden), "GUI host gained {forbidden}");
    }
    for required in [
        "compile_catalog_prepared_group_helper_program(",
        "compile_catalog_prepared_group_source(",
        "prepare_atomic_group_in_scope(",
        "StackedEffectSession::new(",
        "Sequence::start(",
        "Parallel::start(",
        "assemble_group_reply(",
        "impl<'s> HostResultDomain<Ack<'s>> for Schema<'s>",
        "impl ReplyAuthority<MemberIdentity>",
        "Rc::ptr_eq(&schema.result.owner, &self.widget)",
        "std::ptr::eq(branch.result_type.0, &schema.result)",
        "assert_eq!(prepared_fuel, 948)",
        "assert_eq!(final_fuel, 925)",
        "for fuel in 52..=75",
        "parsed_gui_helpers_return_heterogeneous_acks_through_seq_all_and_one_enclosing_meter",
        "gui_member_reply_rejections_preserve_original_payload_and_pending_sequence",
        "gui_generation_revocation_or_cancel_blocks_next_publication_without_undoing_first_mutation",
        "gui_accepted_group_cancel_or_stale_generation_never_projects_or_returns_to_caller",
        "gui_cold_types_missing_grants_preparation_refusal_and_exhaustion_never_invoke",
        "gui_prepared_parallel_data_is_not_a_grant_to_invoke_after_revocation",
        "gui_exhaustion_at_every_enclosing_reentry_debit_seals_without_replaying_mutations",
        "scoped_gui_expression_reuses_one_compiled_tree_with_fresh_inputs_and_original_reply_sessions",
        "scoped_gui_expression_runtime_inputs_are_not_authorized_by_compile_time_types",
        "scoped_gui_expression_cancellation_and_revocation_fence_captured_external_inputs",
    ] {
        assert!(source.contains(required), "GUI host lost {required}");
    }
    let invocation = source
        .split("    fn invoke(")
        .nth(1)
        .unwrap()
        .split("impl<'ir, 's> HostResultDomain")
        .next()
        .unwrap();
    assert!(
        invocation.find("self.admit_prepared(").unwrap()
            < invocation.find("self.widget.text.replace(").unwrap()
    );
    for native in ["Key", "Field", "Domain", "Declaration", "Input"] {
        assert!(!source.contains(&format!("impl Clone for {native}")));
        assert!(!source.contains(&format!("impl std::fmt::Debug for {native}")));
    }
}

#[test]
fn shared_gui_ir_proof_owns_execution_domains_instead_of_generation_only_receipts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_gui_host.rs"))
            .unwrap();
    for required in [
        "struct ExecutionIdentity(LocalExecutionLease)",
        "self.0.same_scope(&other.0)",
        "struct MemberIdentity",
        "struct MemberDomain",
        "HostResultDomain<Ack<'s>> for MemberDomain",
        "self.identity != ack.identity",
        "HostResultDomain<Collected<'ir, 's>> for Execution",
        "self.identity != reply.identity",
        "self.execution.correlated.replace(true)",
        "self.authorize(&request.request.identity)?",
        "left_scope.close()",
        "shared_gui_ir_interleaves_scoped_executions_without_cross_accepting_receipts",
        "shared_gui_parallel_barriers_reject_foreign_receipts_before_consuming_pending_members",
        "gui_execution_cancellation_fences_late_dispatch_without_revoking_other_sessions",
        "gui_execution_domains_are_single_use_and_foreign_dispatch_cannot_invoke",
    ] {
        assert!(
            source.contains(required),
            "missing execution isolation proof {required}"
        );
    }
    assert!(!source.contains("identity: (GENERATION, index)"));
    assert!(!source.contains("type Identity = u64"));
    for native in ["ExecutionIdentity", "MemberIdentity", "Invocation"] {
        assert!(!source.contains(&format!("impl Clone for {native}")));
    }
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Shared IR Execution Isolation"));
    assert!(docs.contains("not a cross-thread scheduler or a wire authentication protocol"));
}

#[test]
fn local_execution_ownership_is_shared_by_the_native_gui_without_a_second_session() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let core =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/local_execution.rs"))
            .unwrap();
    for required in [
        "pub struct LocalExecutionScope",
        "pub struct LocalExecutionLease",
        "pub struct ScopedLocalExecution<Value>",
        "Weak<Cell<bool>>",
        "Weak::ptr_eq",
        "impl Drop for LocalExecutionScope",
        "drop(scope)",
    ] {
        assert!(
            core.contains(required),
            "missing native ownership primitive {required}"
        );
    }
    for forbidden in [
        "leselang_hir",
        "leselang_vm",
        "leserpent",
        "unsafe ",
        "Mutex",
        "Atomic",
        "serde",
        "tokio",
    ] {
        assert!(!core.contains(forbidden), "local fence gained {forbidden}");
    }
    let native = std::fs::read_to_string(root.join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(native.contains("pub use leselang_runtime_core as core"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_gui_host.rs"))
            .unwrap();
    for required in [
        "LocalExecutionScope::new()",
        "scope.bind(StackedEffectSession::new(",
        "guarded_gui_owner_drop_or_unwind_revokes_escaped_work_without_poisoning_reuse",
        "guarded_gui_owner_drop_rejects_late_receipt_without_consuming_pending_member",
        "guarded_gui_explicit_close_fences_accepted_reentry_without_resetting_core_state",
    ] {
        assert!(
            proof.contains(required),
            "GUI adapter lost shared lifetime proof {required}"
        );
    }
    assert!(!proof.contains("live: Cell<bool>"));
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Guarded Native Execution Lifetime"));
    assert!(docs.contains("Full host parity, durable recovery and extraction stay open"));
}

#[test]
fn local_vm_terminal_liveness_delegates_to_the_original_session_without_error_inference() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let local =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/local_effect_session.rs"))
            .unwrap();
    for required in [
        "LocalExecutionScope",
        "StackedControlSession::new(",
        "self.session.poll_in_scope(environment, &self.scope)",
        ".try_accept_in_scope(identity, reply, authority, &self.scope)",
        "self.session.cancel_in_scope(&self.scope)",
    ] {
        assert!(local.contains(required), "local entry lost {required}");
    }
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_stack_session.rs"))
            .unwrap();
    for required in [
        "session.poll_with_scope(environment, Some(scope))",
        ".try_accept_with_scope(identity, reply, authority, Some(scope))",
        "matches!(self.status(), EffectSessionStatus::Terminal(_))",
        "local_scope::CallFence",
        "scope.close()",
        "fence.finish()",
    ] {
        assert!(
            shared.contains(required),
            "shared local entry lost {required}"
        );
    }
    for forbidden in [
        "enum State",
        "SessionCore::new",
        "evaluate_stacked_effects",
        "Fuel::new",
        "catch_unwind",
        "error.code",
        "unsafe ",
        "leserpent",
        "tokio",
    ] {
        assert!(!local.contains(forbidden), "local entry added {forbidden}");
    }
    let native = std::fs::read_to_string(root.join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(native.contains(
        "pub use leselang_hir::local_effect_session::LocalStackedControlSession as LocalVm;"
    ));
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_gui_host.rs"))
        .unwrap();
    for proof in [
        "local_gui_session_closes_on_completion_but_preserves_recoverable_reply_rejection",
        "local_gui_session_normal_failure_and_preclosed_scope_never_leave_live_authority",
        "local_gui_session_caught_poll_and_accept_unwind_revoke_a_retained_owner",
        "local_gui_session_cancel_and_drop_fence_already_published_native_requests",
    ] {
        assert!(gui.contains(proof), "missing local session proof {proof}");
    }
    let facade =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_embedding.rs")).unwrap();
    assert!(facade.contains(
        "native_local_vm_revokes_before_initial_value_cleanup_and_keeps_cancel_sticky_on_unwind"
    ));
    assert!(facade.contains("opt_in_local_vm_does_not_remove_send_from_the_original_generic_vm"));
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Local Session Entry"));
    assert!(docs.contains("callback-local\nor consumed-frame cleanup"));
}

#[test]
fn fenced_host_work_reuses_pending_state_and_exports_the_same_native_proofs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let core = std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_session.rs"))
        .unwrap();
    let start = core
        .find("pub(crate) fn with_host_work<Input, Output>")
        .unwrap();
    let body = &core[start..core[start..].find("/// Shared identity").unwrap() + start];
    for required in [
        "self.status() != EffectSessionStatus::Awaiting",
        "return Err(input)",
        "State::Terminal(EffectSessionEnd::HostUncertain)",
        "self.with_metered_host_work(input, |input, _| work(input))",
        "let output = work(input, &mut self.fuel)",
        "self.state = pending",
    ] {
        assert!(body.contains(required), "host boundary lost {required}");
    }
    for forbidden in [
        "fuel.charge",
        "Fuel::new",
        "catch_unwind",
        ".clone()",
        "error.code",
    ] {
        assert!(!body.contains(forbidden), "host boundary added {forbidden}");
    }
    let local =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/local_effect_session.rs"))
            .unwrap();
    assert!(local.contains("pub fn with_host_work<Input, Output>"));
    assert!(local.contains(".with_host_work_in_scope(input, work, &self.scope)"));
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_stack_session.rs"))
            .unwrap();
    assert!(shared.contains("session.core.with_host_work(input, |input|"));
    assert!(
        shared.contains(
            "self.with_host_outcome_in_scope(input, |input| HostWorkOutcome::Pending(work(input)), scope)"
        )
    );
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_gui_host.rs"))
        .unwrap();
    assert!(
        gui.contains(
            "local_gui_host_work_unwind_seals_retained_session_and_blocks_escaped_siblings"
        )
    );
    assert!(gui.contains(
        "local_gui_host_work_cannot_run_before_dispatch_after_acceptance_or_after_completion"
    ));
    let native =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_embedding.rs")).unwrap();
    assert!(
        native.contains("native_local_host_work_unwind_revokes_before_pending_identity_cleanup")
    );
    assert!(
        native.contains(
            "native_local_host_work_preserves_refused_move_only_input_and_borrowed_outputs"
        )
    );
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Fenced Native Host Work"));
    assert!(docs.contains("same SessionCore pending"));
    assert!(docs.contains("full extraction gate"));
}

#[test]
fn local_poll_fences_retained_owners_through_shared_execution_without_catching_language_faults() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fence =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/local_scope.rs")).unwrap();
    for required in [
        "Option<&'scope LocalExecutionScope>",
        "impl Drop for CallFence",
        "if output.is_err()",
        "fence.finish()",
    ] {
        assert!(fence.contains(required));
    }
    for forbidden in [
        "catch_unwind",
        "enum State",
        "Fuel",
        "unsafe ",
        "pub fn",
        "serde",
    ] {
        assert!(
            !fence.contains(forbidden),
            "private fence gained {forbidden}"
        );
    }
    let stack =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_stack.rs")).unwrap();
    assert!(stack.contains(
        "resume_accepted_stacked_effects_with_scope(accepted, environment, fuel, policy, None)"
    ));
    let body = stack
        .split_once("pub(crate) fn resume_accepted_stacked_effects_with_scope")
        .unwrap()
        .1;
    let output = body.find("let output =").unwrap();
    let cleanup = body.find("let cleanup_fence = fence;").unwrap();
    let revoke = body.find("cleanup_fence.close()").unwrap();
    let dispose = body
        .find("drop((current, frames, reply, returned, identity))")
        .unwrap();
    assert!(output < cleanup && cleanup < revoke && revoke < dispose);
    assert!(body.contains("CallFence::run(execution_scope"));
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "local_reentry_revokes_retained_owners_before_failure_cleanup",
        "local_correlation_failure_closes_retained_successor_frames_before_cleanup",
        "local_invalid_restored_prefix_revokes_before_owned_value_disposal",
        "local_owned_prefix_cleanup_follows_terminal_slice_liveness",
        "local_identity_cleanup_unwind_revokes_before_computed_output_disposal",
    ] {
        assert!(tests.contains(proof));
    }
    let native =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_embedding.rs")).unwrap();
    assert!(
        native.contains("native_local_reentry_failure_revokes_before_retained_identity_cleanup")
    );
    assert!(native.contains(
        "native_local_recovered_calculation_keeps_successor_live_and_exact_unscoped_fuel"
    ));
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Fenced Reentry Cleanup"));
    assert!(docs.contains("Complete\nhost/recovery/extraction gates remain open"));
}

#[test]
fn local_acceptance_fences_shared_native_cleanup_without_replacing_validation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let reply =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/reply.rs")).unwrap();
    assert!(reply.contains("pub fn try_accept_in_scope<Reply, View: ?Sized, Authority>"));
    assert!(reply.contains("self.try_accept_with_scope(actual_identity, reply, authority, None)"));
    let body = reply.split_once("fn try_accept_with_scope").unwrap().1;
    let own = body.find("State::Waiting(waiting) => waiting").unwrap();
    let fence = body.find("let mut fence = AcceptanceFence(scope)").unwrap();
    let check = body.find("&waiting.identity != actual_identity").unwrap();
    let disarm = body.find("fence.0 = None").unwrap();
    let restore = body.find("self.state = State::Waiting(waiting)").unwrap();
    assert!(own < fence && fence < check && check < disarm && disarm < restore);
    for forbidden in [
        "catch_unwind",
        "unsafe ",
        "use serde",
        "Fuel::new",
        "leserpent",
    ] {
        assert!(!reply.contains(forbidden), "acceptance added {forbidden}");
    }
    let session =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_session.rs"))
            .unwrap();
    assert_eq!(
        session
            .matches("pending.try_accept_in_scope(identity, reply, authority, scope)")
            .count(),
        2
    );
    let native =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_embedding.rs")).unwrap();
    assert!(native.contains(
        "native_local_acceptance_revokes_before_pending_cleanup_but_not_normal_rejection"
    ));
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/tests/reply.rs")).unwrap();
    for proof in [
        "scoped_acceptance_unwind_revokes_before_owned_identity_frame_and_reply_cleanup",
        "scoped_normal_rejection_preserves_original_input_domain_and_live_retry",
        "preclosed_acceptance_scope_cancels_before_cleanup_without_entering_callbacks",
        "scoped_native_callback_revocation_stops_before_the_next_phase_and_returns_input",
        "scoped_revocation_seals_cancelled_before_frame_or_native_error_cleanup_unwind",
        "unscoped_acceptance_does_not_infer_an_unrelated_scope_policy",
        "scoped_and_unscoped_entries_preserve_the_original_native_inequality_hook",
    ] {
        assert!(tests.contains(proof));
    }
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Fenced Reply Acceptance"));
    assert!(docs.contains("#### Synchronous Revocation During Acceptance"));
    assert!(docs.contains("full host/recovery/extraction gates remain open"));
}

#[test]
fn scoped_reply_revocation_reuses_original_domain_validation_and_cannot_rearm_sessions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let reply =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/reply.rs")).unwrap();
    assert!(reply.contains("struct LiveResultDomain<'a, Domain: ?Sized>"));
    assert!(reply.contains(
        "self.domain.matches_type(view) && self.scope.is_none_or(LocalExecutionScope::is_open)"
    ));
    assert!(reply.contains("self.domain.validate_value(view)"));
    let body = reply.split_once("fn try_accept_with_scope").unwrap().1;
    let close = body
        .find("self.state = State::Terminal(ReplyEnd::Cancelled)")
        .unwrap();
    assert!(close < body.find("drop(error)").unwrap());
    assert!(close < body.find("drop(waiting)").unwrap());
    assert_eq!(body.matches("validate_host_result(").count(), 1);
    let core = std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_session.rs"))
        .unwrap();
    let acceptance = core
        .split_once("pub(crate) fn try_accept_with_scope")
        .unwrap()
        .1
        .split_once("impl<Node:")
        .unwrap()
        .0;
    assert_eq!(
        acceptance
            .matches("ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)")
            .count(),
        2
    );
    assert_eq!(
        acceptance
            .matches("State::Terminal(EffectSessionEnd::Cancelled)")
            .count(),
        2
    );
    assert!(
        core.contains("scoped_cancelled_rejection_never_rearms_binding_or_final_pending_state")
    );
}

#[test]
fn scoped_group_entries_reuse_original_reply_cores_without_storing_scope_or_refilling_fuel() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in [
        "sequence_evaluation",
        "parallel_evaluation",
        "prepared_sequence",
        "prepared_parallel",
    ] {
        let source =
            std::fs::read_to_string(root.join(format!("crates/leselang-hir-core/src/{file}.rs")))
                .unwrap();
        assert!(source.contains("pub fn try_accept_in_scope<"));
        assert!(source.contains("pub fn poll_in_scope"));
        assert!(source.contains("scope: &LocalExecutionScope"));
        assert!(source.contains("try_accept_with_scope("));
        for forbidden in [
            "scope: LocalExecutionScope",
            "scope: Option<LocalExecutionScope>",
            "catch_unwind",
            "Fuel::new",
        ] {
            assert!(!source.contains(forbidden), "{file} gained {forbidden}");
        }
        if !file.starts_with("prepared_") {
            assert_eq!(source.matches("enum State<").count(), 1);
            let acceptance = source
                .split_once("pub(crate) fn try_accept_with_scope")
                .unwrap()
                .1
                .split_once("pub(crate) fn cancel")
                .unwrap()
                .0;
            assert!(
                acceptance
                    .contains("pending.try_accept_in_scope(identity, reply, authority, scope)")
            );
            assert!(acceptance.contains("ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)"));
            assert!(
                acceptance.find("self.cancel()").unwrap()
                    < acceptance.find("let status = self.status()").unwrap()
            );
            assert!(!acceptance.contains("fuel.charge"));
            let publish = source
                .split_once("pub(crate) fn publish_with<")
                .unwrap()
                .1
                .split_once("pub(crate) fn try_accept<")
                .unwrap()
                .0;
            assert!(publish.contains("scope: Option<&LocalExecutionScope>"));
            assert!(publish.contains("CallFence::new(scope)"));
            assert!(
                publish.find("self.cancel()").unwrap() < publish.find("HostUncertain").unwrap()
            );
            assert!(publish.contains("scope.is_some_and(|scope| !scope.is_open())"));
        }
    }
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scoped_group_acceptance.rs"))
            .unwrap();
    for proof in [
        "all_four_entries_cancel_at_each_native_phase_without_rearming_or_refilling",
        "scoped_refusal_preserves_original_input_domain_rows_prefix_and_ordered_completion",
        "preclosed_scope_cancels_even_invalid_member_or_ready_results_without_native_hooks",
        "scoped_callback_unwind_revokes_before_pending_suffix_and_accepted_sibling_cleanup",
        "scoped_cancellation_seal_survives_retained_owner_cleanup_unwind",
        "unscoped_collectors_do_not_infer_an_unrelated_scope_policy",
        "scoped_poll_preclosed_ready_waiting_prefix_and_ready_results_never_publish_or_charge",
        "scoped_publication_revocation_stops_after_original_callback_without_partial_batch_or_refund",
        "scoped_poll_failure_and_unwind_revoke_before_retained_prefix_or_suffix_cleanup",
        "scoped_poll_cleanup_unwind_preserves_cancelled_or_conservative_uncertain_reason",
        "scoped_prepared_poll_seals_before_superseded_error_cleanup_and_preserves_normal_error",
        "scoped_and_unscoped_successful_polling_keep_original_rows_requests_and_exact_fuel",
        "unscoped_polling_does_not_adopt_an_unrelated_closed_scope",
        "scoped_poll_exhaustion_closes_without_preparation_or_refund",
    ] {
        assert!(tests.contains(proof));
    }
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_gui_host.rs"))
        .unwrap();
    assert!(gui.contains(
        "scoped_gui_collector_revocation_stops_group_delivery_without_cancelling_a_sibling"
    ));
    assert!(gui.contains(
        "scoped_gui_poll_cancellation_blocks_next_publication_and_completed_barrier_handoff"
    ));
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("#### Scoped Group Acceptance"));
    assert!(docs.contains("#### Scoped Group Polling"));
    assert!(docs.contains("LocalVm still owns its scope\nprivately"));
}

#[test]
fn classified_native_outcomes_reuse_core_terminal_states_without_error_string_inference() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let core = std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_session.rs"))
        .unwrap();
    for required in [
        "pub enum HostWorkOutcome<Output>",
        "HostWorkOutcome::Pending(output)",
        "HostWorkOutcome::Cancelled(output)",
        "HostWorkOutcome::Failed(output)",
        "HostWorkOutcome::Uncertain(output)",
        "self.state = State::Terminal(EffectSessionEnd::Failed)",
        "self.state = State::Terminal(EffectSessionEnd::Cancelled)",
    ] {
        assert!(
            core.contains(required),
            "missing native outcome boundary {required}"
        );
    }
    let local =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/local_effect_session.rs"))
            .unwrap();
    assert!(local.contains("pub fn with_host_outcome<Input, Output>"));
    assert!(local.contains(".with_host_outcome_in_scope(input, work, &self.scope)"));
    let shared =
        std::fs::read_to_string(root.join("crates/leselang-hir-core/src/effect_stack_session.rs"))
            .unwrap();
    let classified = shared
        .split_once("pub fn with_host_outcome_in_scope<Input, Output>")
        .unwrap()
        .1
        .split_once("pub fn with_metered_host_outcome_in_scope<Input, Output>")
        .unwrap()
        .0;
    let outcome = classified.find("let output = work(input)").unwrap();
    let terminal = classified
        .find("if !matches!(&output, HostWorkOutcome::Pending(_))")
        .unwrap();
    let revoke = classified.find("scope.close()").unwrap();
    let finish = classified.find("fence.finish()").unwrap();
    assert!(outcome < terminal && terminal < revoke && revoke < finish);
    for forbidden in [
        "enum State",
        "error.code",
        "catch_unwind",
        "Fuel::new",
        "unsafe ",
    ] {
        assert!(
            !local.contains(forbidden),
            "local outcome added {forbidden}"
        );
    }
    let facade = std::fs::read_to_string(root.join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(facade.contains("pub use leselang_hir::effect_session::HostWorkOutcome;"));
    let native =
        std::fs::read_to_string(root.join("crates/leselang-vm/tests/native_embedding.rs")).unwrap();
    assert!(native.contains(
        "native_classified_host_failure_retains_output_and_terminal_reason_through_cleanup_unwind"
    ));
    assert!(native.contains(
        "native_host_outcome_debug_never_observes_or_requires_native_payload_formatting"
    ));
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/prepared_gui_host.rs"))
        .unwrap();
    assert!(gui.contains(
        "classified_gui_failure_and_uncertainty_close_without_rollback_or_sibling_revocation"
    ));
    assert!(
        gui.contains("classified_gui_cancellation_hands_back_partial_collectors_after_revocation")
    );
    let docs = std::fs::read_to_string(root.join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Classified Native Outcomes"));
    assert!(docs.contains("#### Cancellation Handoff"));
    assert!(docs.contains("not operation success"));
    assert!(docs.contains("host/recovery/extraction gates\nremain open"));
}
