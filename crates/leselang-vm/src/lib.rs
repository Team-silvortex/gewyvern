use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

use leselang_hir::{Effect, HirProgram, Type, authorize};
use leserpent_domain::{
    CapabilitySet, DOMAIN_SCHEMA_VERSION, Principal, Query, QueryEnvelope, QueryResult, Revision,
    RuntimeProjection,
};
use serde::{Deserialize, Serialize};

mod journal;

use journal::Journal;
pub use journal::{
    JOURNAL_SCHEMA_VERSION, MAX_JOURNAL_ENTRY_BYTES, MAX_JOURNAL_RECORDS, MAX_JOURNAL_TOTAL_BYTES,
};

pub const CONTINUATION_SCHEMA_VERSION: u32 = 1;
pub const MAX_CONTINUATION_BYTES: usize = 64 * 1024;
pub const DEFAULT_FUEL: u64 = 1_000;
pub const MAX_EXECUTION_FUEL: u64 = 1_000_000;
pub const DEFAULT_EFFECT_DEADLINE_MS: u64 = 30_000;
pub const MAX_EFFECT_DEADLINE_MS: u64 = 24 * 60 * 60 * 1_000;
pub const DEFAULT_MAX_OUTPUT_ITEMS: usize = 10_000;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ContinuationToken(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContinuationImage {
    pub schema_version: u32,
    pub token: ContinuationToken,
    pub program_counter: u32,
    pub expected_revision: Option<Revision>,
    pub result_type: Type,
    pub pending_effect: Effect,
    pub fuel_remaining: u64,
    pub deadline_ms: u64,
    pub max_output_items: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceBudget {
    pub fuel_remaining: u64,
    pub deadline_ms: u64,
    pub max_output_items: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EffectRequest {
    pub effect_id: String,
    pub required_capability: String,
    pub query: QueryEnvelope,
    pub continuation: ContinuationImage,
    pub budget: ResourceBudget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Value {
    RuntimeList {
        revision: Revision,
        runtimes: Vec<RuntimeProjection>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Fault {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum Step {
    Done(Value),
    Effect(Box<EffectRequest>),
    Yield(ContinuationImage),
    Fault(Fault),
}

pub struct Vm {
    next_effect_id: u64,
    fuel_limit: u64,
    pending: BTreeMap<ContinuationToken, ContinuationImage>,
    completed: BTreeMap<ContinuationToken, Step>,
    journal: Journal,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(DEFAULT_FUEL)
    }
}

impl Vm {
    pub fn new(fuel_limit: u64) -> Self {
        Self {
            next_effect_id: 1,
            fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
            pending: BTreeMap::new(),
            completed: BTreeMap::new(),
            journal: Journal::ephemeral(),
        }
    }

    pub fn open_journal(path: impl AsRef<Path>, fuel_limit: u64) -> Result<Self, Fault> {
        let (journal, snapshot) = Journal::open(path.as_ref())?;
        Ok(Self {
            next_effect_id: snapshot.next_sequence,
            fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
            pending: snapshot.pending,
            completed: snapshot.completed,
            journal,
        })
    }

    pub fn start(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
    ) -> Step {
        if self.fuel_limit == 0 {
            return fault("LSV1001", "execution fuel exhausted");
        }
        if let Err(error) = authorize(program, &capabilities) {
            return fault(&error.code, error.message);
        }

        let sequence = match self.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        let token = ContinuationToken(format!("continuation-{sequence}"));
        let image = ContinuationImage {
            schema_version: CONTINUATION_SCHEMA_VERSION,
            token: token.clone(),
            program_counter: 1,
            expected_revision,
            result_type: program.function.result_type,
            pending_effect: program.function.effect.clone(),
            fuel_remaining: self.fuel_limit - 1,
            deadline_ms: DEFAULT_EFFECT_DEADLINE_MS,
            max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
        };
        if let Err(error) = encode_continuation(&image) {
            return Step::Fault(error);
        }
        let query = match &program.function.effect {
            Effect::RuntimeList { filter } => Query::RuntimeList {
                filter: filter.clone(),
            },
        };
        if let Err(error) = self.journal.record_pending(&image) {
            return Step::Fault(error);
        }
        self.pending.insert(token, image.clone());
        Step::Effect(Box::new(EffectRequest {
            effect_id: format!("effect-{sequence}"),
            required_capability: program.function.required_capability.clone(),
            query: QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal,
                capabilities,
                query,
            },
            continuation: image.clone(),
            budget: ResourceBudget {
                fuel_remaining: self.fuel_limit - 1,
                deadline_ms: image.deadline_ms,
                max_output_items: image.max_output_items,
            },
        }))
    }

    pub fn restore(&mut self, image: ContinuationImage) -> Result<(), Fault> {
        validate_image(&image)?;
        if let Some(current) = self.pending.get(&image.token) {
            return if current == &image {
                Ok(())
            } else {
                Err(Fault {
                    code: "LSV2002".to_string(),
                    message: "continuation token conflicts with pending state".to_string(),
                })
            };
        }
        if self.completed.contains_key(&image.token) {
            return Err(Fault {
                code: "LSV2003".to_string(),
                message: "continuation was already consumed".to_string(),
            });
        }
        if let Some(sequence) = image
            .token
            .as_str()
            .strip_prefix("continuation-")
            .and_then(|value| value.parse::<u64>().ok())
        {
            self.next_effect_id = self.next_effect_id.max(sequence.saturating_add(1));
        }
        self.journal.record_pending(&image)?;
        self.pending.insert(image.token.clone(), image);
        Ok(())
    }

    pub fn resume(&mut self, image: &ContinuationImage, result: QueryResult) -> Step {
        if let Some(completed) = self.completed.get(&image.token) {
            return completed.clone();
        }
        if let Err(error) = validate_image(image) {
            return Step::Fault(error);
        }
        let Some(stored) = self.pending.get(&image.token) else {
            return fault("LSV2004", "unknown continuation token");
        };
        if stored != image {
            return fault("LSV2005", "continuation image does not match pending state");
        }

        let step = match (&image.pending_effect, image.result_type, result) {
            (
                Effect::RuntimeList { .. },
                Type::RuntimeList,
                QueryResult::RuntimeList { revision, runtimes },
            ) => {
                if let Some(expected) = image.expected_revision
                    && expected != revision
                {
                    fault(
                        "LSV2101",
                        format!(
                            "effect revision conflict: expected {}, actual {}",
                            expected.0, revision.0
                        ),
                    )
                } else if runtimes.len() > image.max_output_items {
                    fault(
                        "LSV2102",
                        format!(
                            "effect returned {} items, limit is {}",
                            runtimes.len(),
                            image.max_output_items
                        ),
                    )
                } else {
                    Step::Done(Value::RuntimeList { revision, runtimes })
                }
            }
        };
        let authoritative = match self.journal.record_completed(image, &step) {
            Ok(step) => step,
            Err(error) => return Step::Fault(error),
        };
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        authoritative
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn pending_continuations(&self) -> Vec<ContinuationImage> {
        self.pending.values().cloned().collect()
    }

    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        loop {
            let sequence = self.journal.allocate_sequence(self.next_effect_id)?;
            self.next_effect_id = self.next_effect_id.saturating_add(1);
            let token = ContinuationToken(format!("continuation-{sequence}"));
            if !self.pending.contains_key(&token) && !self.completed.contains_key(&token) {
                return Ok(sequence);
            }
        }
    }
}

impl ContinuationToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn encode_continuation(image: &ContinuationImage) -> Result<Vec<u8>, Fault> {
    validate_image(image)?;
    encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")
}

fn encode_json_capped<T: Serialize>(
    value: &T,
    limit: usize,
    label: &str,
) -> Result<Vec<u8>, Fault> {
    let mut writer = CappedWriter::new(limit);
    if let Err(error) = serde_json::to_writer(&mut writer, value) {
        if writer.overflowed {
            return Err(Fault {
                code: "LSV3002".to_string(),
                message: format!("{label} exceeds {limit} bytes"),
            });
        }
        return Err(Fault {
            code: "LSV3001".to_string(),
            message: format!("failed to encode {label}: {error}"),
        });
    }
    Ok(writer.output)
}

pub fn decode_continuation(bytes: &[u8]) -> Result<ContinuationImage, Fault> {
    if bytes.len() > MAX_CONTINUATION_BYTES {
        return Err(Fault {
            code: "LSV3002".to_string(),
            message: format!("continuation exceeds {MAX_CONTINUATION_BYTES} bytes"),
        });
    }
    let image = serde_json::from_slice(bytes).map_err(|error| Fault {
        code: "LSV3003".to_string(),
        message: format!("invalid continuation: {error}"),
    })?;
    validate_image(&image)?;
    Ok(image)
}

fn validate_image(image: &ContinuationImage) -> Result<(), Fault> {
    if image.schema_version != CONTINUATION_SCHEMA_VERSION {
        return Err(Fault {
            code: "LSV2001".to_string(),
            message: format!(
                "unsupported continuation version {}, expected {}",
                image.schema_version, CONTINUATION_SCHEMA_VERSION
            ),
        });
    }
    if image.token.0.is_empty()
        || image.token.0.len() > 128
        || !image
            .token
            .0
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(Fault {
            code: "LSV2006".to_string(),
            message: "invalid continuation token".to_string(),
        });
    }
    if image.fuel_remaining > MAX_EXECUTION_FUEL {
        return Err(Fault {
            code: "LSV2007".to_string(),
            message: "continuation fuel exceeds runtime limit".to_string(),
        });
    }
    if image.deadline_ms == 0 || image.deadline_ms > MAX_EFFECT_DEADLINE_MS {
        return Err(Fault {
            code: "LSV2008".to_string(),
            message: "continuation deadline exceeds runtime limit".to_string(),
        });
    }
    if image.max_output_items > DEFAULT_MAX_OUTPUT_ITEMS {
        return Err(Fault {
            code: "LSV2009".to_string(),
            message: "continuation output limit exceeds runtime maximum".to_string(),
        });
    }
    Ok(())
}

struct CappedWriter {
    output: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl CappedWriter {
    fn new(limit: usize) -> Self {
        Self {
            output: Vec::new(),
            limit,
            overflowed: false,
        }
    }
}

impl Write for CappedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.output.len().saturating_add(buffer.len()) > self.limit {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "continuation exceeds limit",
            ));
        }
        self.output.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fault(code: &str, message: impl Into<String>) -> Step {
    Step::Fault(Fault {
        code: code.to_string(),
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leserpent_domain::{
        CAPABILITY_RUNTIME_READ, InMemoryControlPlane, RuntimeId, RuntimeListFilter,
    };

    use super::*;

    static NEXT_TEMP_JOURNAL: AtomicU64 = AtomicU64::new(1);

    struct TempJournal {
        root: PathBuf,
        path: PathBuf,
    }

    impl TempJournal {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEMP_JOURNAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "leselang-vm-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("effects.sqlite3");
            Self { root, path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempJournal {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn program() -> HirProgram {
        lower(&parse(
            "fn main() = runtime.list(environment: \"production\", role: none)",
        ))
        .unwrap()
    }

    fn start(vm: &mut Vm, expected_revision: Option<Revision>) -> EffectRequest {
        let Step::Effect(request) = vm.start(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision,
        ) else {
            panic!("expected effect");
        };
        *request
    }

    #[test]
    fn stackless_start_emits_typed_runtime_list_effect() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(7)));
        assert_eq!(request.required_capability, CAPABILITY_RUNTIME_READ);
        assert_eq!(request.continuation.program_counter, 1);
        assert_eq!(vm.pending_count(), 1);
        let Query::RuntimeList { filter } = request.query.query;
        assert_eq!(
            filter,
            RuntimeListFilter {
                environment: Some("production".to_string()),
                cluster: None,
                role: None,
            }
        );
    }

    #[test]
    fn completion_consumes_once_and_duplicate_delivery_is_idempotent() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(7)));
        let result = QueryResult::RuntimeList {
            revision: Revision(7),
            runtimes: Vec::new(),
        };
        let first = vm.resume(&request.continuation, result.clone());
        let duplicate = vm.resume(&request.continuation, result);
        assert_eq!(first, duplicate);
        assert!(matches!(first, Step::Done(Value::RuntimeList { .. })));
        assert_eq!(vm.pending_count(), 0);
    }

    #[test]
    fn continuation_survives_serialization_and_process_restart() {
        let mut first_vm = Vm::default();
        let request = start(&mut first_vm, None);
        let encoded = encode_continuation(&request.continuation).unwrap();
        let restored_image = decode_continuation(&encoded).unwrap();

        let mut restarted_vm = Vm::default();
        restarted_vm.restore(restored_image.clone()).unwrap();
        let step = restarted_vm.resume(
            &restored_image,
            QueryResult::RuntimeList {
                revision: Revision(11),
                runtimes: Vec::new(),
            },
        );
        assert!(matches!(
            step,
            Step::Done(Value::RuntimeList {
                revision: Revision(11),
                ..
            })
        ));
    }

    #[test]
    fn revision_conflict_is_typed_and_idempotently_replayed() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(3)));
        let result = QueryResult::RuntimeList {
            revision: Revision(4),
            runtimes: Vec::new(),
        };
        let first = vm.resume(&request.continuation, result.clone());
        let duplicate = vm.resume(&request.continuation, result);
        assert_eq!(first, duplicate);
        assert!(matches!(first, Step::Fault(Fault { ref code, .. }) if code == "LSV2101"));
    }

    #[test]
    fn missing_capability_and_invalid_images_fail_without_effects() {
        let mut vm = Vm::default();
        let step = vm.start(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::default(),
            None,
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSH2001"));
        assert_eq!(vm.pending_count(), 0);

        let mut image = start(&mut vm, None).continuation;
        image.schema_version = 99;
        assert_eq!(encode_continuation(&image).unwrap_err().code, "LSV2001");
        assert_eq!(
            decode_continuation(&vec![b' '; MAX_CONTINUATION_BYTES + 1])
                .unwrap_err()
                .code,
            "LSV3002"
        );

        let mut forged_budget = start(&mut vm, None).continuation;
        forged_budget.deadline_ms = 0;
        assert_eq!(
            encode_continuation(&forged_budget).unwrap_err().code,
            "LSV2008"
        );
        forged_budget.deadline_ms = DEFAULT_EFFECT_DEADLINE_MS;
        forged_budget.max_output_items = DEFAULT_MAX_OUTPUT_ITEMS + 1;
        assert_eq!(
            encode_continuation(&forged_budget).unwrap_err().code,
            "LSV2009"
        );
    }

    #[test]
    fn source_to_hir_to_effect_to_domain_to_value_is_one_vertical_slice() {
        let mut vm = Vm::default();
        let unfiltered = lower(&parse("fn main() = runtime.list()")).unwrap();
        let Step::Effect(request) = vm.start(
            &unfiltered,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        ) else {
            panic!("expected effect");
        };
        let request = *request;
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-b").unwrap(), "Bravo", "http://b");
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "Alpha", "http://a");

        let result = control.query(request.query).unwrap();
        let step = vm.resume(&request.continuation, result);
        let Step::Done(Value::RuntimeList { runtimes, .. }) = step else {
            panic!("vertical slice should complete");
        };
        assert_eq!(runtimes[0].id.as_str(), "runtime-a");
        assert_eq!(runtimes[1].id.as_str(), "runtime-b");
    }

    #[test]
    fn restored_tokens_do_not_collide_with_new_effects() {
        let mut first = Vm::default();
        let restored = start(&mut first, None).continuation;
        let mut restarted = Vm::default();
        restarted.restore(restored).unwrap();
        let next = start(&mut restarted, None);
        assert_eq!(next.continuation.token.as_str(), "continuation-2");
    }

    #[test]
    fn output_and_serialized_continuation_limits_are_enforced() {
        let mut first = Vm::default();
        let mut image = start(&mut first, None).continuation;
        image.max_output_items = 0;
        let mut restarted = Vm::default();
        restarted.restore(image.clone()).unwrap();
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "Alpha", "http://a");
        let result = control
            .query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList {
                    filter: RuntimeListFilter::default(),
                },
            })
            .unwrap();
        assert!(matches!(
            restarted.resume(&image, result),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2102"
        ));

        let large_value = "x".repeat(MAX_CONTINUATION_BYTES);
        let source = format!("fn main() = runtime.list(environment: \"{large_value}\")");
        let large_program = lower(&parse(&source)).unwrap();
        let step = Vm::default().start(
            &large_program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSV3002"));
    }

    #[test]
    fn durable_journal_recovers_pending_and_replays_completion_after_restart() {
        let journal = TempJournal::new("restart");
        let request = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let request = start(&mut first, None);
            assert_eq!(first.pending_count(), 1);
            request
        };

        let first_step = {
            let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            assert_eq!(restarted.pending_count(), 1);
            assert_eq!(
                restarted.pending_continuations(),
                vec![request.continuation.clone()]
            );
            restarted.resume(
                &request.continuation,
                QueryResult::RuntimeList {
                    revision: Revision(21),
                    runtimes: Vec::new(),
                },
            )
        };
        assert!(matches!(
            first_step,
            Step::Done(Value::RuntimeList {
                revision: Revision(21),
                ..
            })
        ));

        let mut restarted_again = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(restarted_again.pending_count(), 0);
        assert_eq!(restarted_again.completed_count(), 1);
        let replay = restarted_again.resume(
            &request.continuation,
            QueryResult::RuntimeList {
                revision: Revision(99),
                runtimes: Vec::new(),
            },
        );
        assert_eq!(replay, first_step);
    }

    #[test]
    fn durable_sequence_allocation_does_not_collide_between_live_vms() {
        let journal = TempJournal::new("sequences");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        let first_request = start(&mut first, None);
        let second_request = start(&mut second, None);

        assert_eq!(first_request.continuation.token.as_str(), "continuation-1");
        assert_eq!(second_request.continuation.token.as_str(), "continuation-2");
        assert_ne!(first_request.effect_id, second_request.effect_id);
    }

    #[test]
    fn concurrent_completion_uses_the_first_durable_result() {
        let journal = TempJournal::new("completion-race");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let request = start(&mut first, None);
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        let first_step = first.resume(
            &request.continuation,
            QueryResult::RuntimeList {
                revision: Revision(31),
                runtimes: Vec::new(),
            },
        );
        let competing_step = second.resume(
            &request.continuation,
            QueryResult::RuntimeList {
                revision: Revision(32),
                runtimes: Vec::new(),
            },
        );

        assert_eq!(competing_step, first_step);
        assert!(matches!(
            competing_step,
            Step::Done(Value::RuntimeList {
                revision: Revision(31),
                ..
            })
        ));
    }

    #[test]
    fn durable_journal_rejects_unknown_schema_versions() {
        let journal = TempJournal::new("version");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 99;")
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4004");
    }

    #[cfg(unix)]
    #[test]
    fn durable_journal_is_private_and_rejects_symbolic_links() {
        use std::os::unix::fs::{MetadataExt, symlink};

        let journal = TempJournal::new("permissions");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        assert_eq!(fs::metadata(journal.path()).unwrap().mode() & 0o777, 0o600);

        let link = journal.root.join("journal-link.sqlite3");
        symlink(journal.path(), &link).unwrap();
        let error = Vm::open_journal(&link, DEFAULT_FUEL).err().unwrap();
        assert_eq!(error.code, "LSV4001");
    }
}
