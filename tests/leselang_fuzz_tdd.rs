use leselang_hir::lower;
use leselang_syntax::{MAX_SOURCE_BYTES, SyntaxTree, TokenKind, format as format_source, parse};
use leselang_vm::{Step, Vm, decode_continuation, encode_continuation};
use leserpent_domain::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH,
    CapabilitySet, Principal,
};

const FUZZ_SEED: u64 = 0x6c65_7365_6c61_6e67;
const SOURCE_CASES: usize = 2_048;
const CONTINUATION_CASES: usize = 2_048;

#[test]
fn deterministic_utf8_parser_hir_vm_fuzz_shelf() {
    let mut random = DeterministicRandom::new(FUZZ_SEED);
    let mut lowered = 0usize;
    let mut formatted = 0usize;
    for source in source_corpus(&mut random) {
        let tree = parse(&source);
        assert_syntax_invariants(&source, &tree);

        let encoded = serde_json::to_vec(&tree).expect("syntax tree must serialize");
        let decoded: SyntaxTree =
            serde_json::from_slice(&encoded).expect("syntax tree must deserialize");
        assert_eq!(decoded, tree, "syntax JSON roundtrip changed `{source}`");
        assert_eq!(
            parse(&source),
            tree,
            "parse was not deterministic for `{source}`"
        );

        if tree.diagnostics.is_empty() {
            let first = format_source(&tree);
            let second = format_source(&tree);
            assert_eq!(first, second, "format was not deterministic for `{source}`");
            if let Ok(canonical) = first {
                formatted += 1;
                assert!(canonical.len() <= MAX_SOURCE_BYTES);
                let reparsed = parse(&canonical);
                assert!(reparsed.diagnostics.is_empty());
                assert_eq!(format_source(&reparsed).unwrap(), canonical);
            }
        }

        let Ok(program) = lower(&tree) else {
            continue;
        };
        lowered += 1;
        let mut vm = Vm::new(4);
        let step = vm.start(
            &program,
            Principal {
                id: "fuzz-operator".to_string(),
            },
            CapabilitySet::new([
                CAPABILITY_RUNTIME_READ,
                CAPABILITY_RUNTIME_REFRESH,
                CAPABILITY_DEBUGGER_CONTROL,
            ]),
            None,
        );
        let step_bytes = serde_json::to_vec(&step).expect("VM step must serialize");
        assert!(
            step_bytes.len() <= MAX_SOURCE_BYTES,
            "VM start step escaped the fuzz output bound"
        );
        assert!(matches!(
            step,
            Step::Done(_) | Step::Effect(_) | Step::Effects(_) | Step::Fault(_)
        ));
    }
    assert!(lowered >= 5, "valid seed programs did not reach the VM");
    assert!(
        formatted >= 5,
        "valid seed programs did not reach the formatter"
    );
    println!(
        "leselang source fuzz valid: seed={FUZZ_SEED} cases={SOURCE_CASES} lowered={lowered} formatted={formatted}"
    );
}

#[test]
fn deterministic_continuation_decoder_fuzz_shelf() {
    let program = lower(&parse("fn main() = runtime.list(environment: \"prod\")")).unwrap();
    let Step::Effect(request) = Vm::new(4).start(
        &program,
        Principal {
            id: "fuzz-operator".to_string(),
        },
        CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
        None,
    ) else {
        panic!("seed program must yield one effect");
    };
    let seed = encode_continuation(&request.continuation).unwrap();
    let program = lower(&parse(r#"fn main() = bind(offset: 1, body: bind(r: runtime.list(), body: add(left: field(value: r, name: "revision"), right: offset)))"#)).unwrap();
    let Step::Effect(request) = Vm::new(100).start(
        &program,
        Principal::new("fuzz-operator").unwrap(),
        CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
        None,
    ) else {
        panic!("result-binding seed must suspend");
    };
    let binding_seed = encode_continuation(&request.continuation).unwrap();
    let mut random = DeterministicRandom::new(FUZZ_SEED ^ 0x564d);
    let mut accepted = 0usize;

    let mut binding_accepted = 0;
    for index in 0..CONTINUATION_CASES {
        let candidate = mutate_bytes(
            if index % 2 == 0 { &seed } else { &binding_seed },
            &mut random,
        );
        let first = decode_continuation(&candidate);
        let second = decode_continuation(&candidate);
        assert_eq!(
            format!("{first:?}"),
            format!("{second:?}"),
            "continuation decoder was not deterministic"
        );
        if let Ok(image) = first {
            accepted += 1;
            if image.result_binding.is_some() {
                binding_accepted += 1;
            }
            let canonical = encode_continuation(&image).unwrap();
            assert_eq!(decode_continuation(&canonical).unwrap(), image);
        }
    }
    assert!(
        accepted > 0,
        "mutation shelf never retained a valid continuation"
    );
    assert!(
        binding_accepted > 0,
        "mutation shelf lost every result-binding continuation"
    );
    assert!(decode_continuation(&vec![b' '; 64 * 1024 + 1]).is_err());
    println!(
        "leselang continuation fuzz valid: seed={} cases={CONTINUATION_CASES} accepted={accepted}",
        FUZZ_SEED ^ 0x564d
    );
}

fn assert_syntax_invariants(source: &str, tree: &SyntaxTree) {
    assert_eq!(tree.source(), source);
    assert!(!tree.tokens.is_empty());
    assert_eq!(tree.tokens.last().unwrap().kind, TokenKind::Eof);

    if source.len() <= MAX_SOURCE_BYTES {
        assert_eq!(tree.reconstruct().as_deref(), Some(source));
        let mut cursor = 0usize;
        for token in tree
            .tokens
            .iter()
            .filter(|token| token.kind != TokenKind::Eof)
        {
            assert_eq!(token.span.start, cursor, "lexer left a source gap");
            assert_span(source, token.span.start, token.span.end);
            cursor = token.span.end;
        }
        assert_eq!(cursor, source.len(), "lexer did not consume the source");
    }
    for diagnostic in &tree.diagnostics {
        assert!(!diagnostic.code.is_empty());
        assert!(!diagnostic.message.is_empty());
        assert_span(source, diagnostic.span.start, diagnostic.span.end);
    }
}

fn assert_span(source: &str, start: usize, end: usize) {
    assert!(start <= end && end <= source.len());
    assert!(source.is_char_boundary(start));
    assert!(source.is_char_boundary(end));
}

fn source_corpus(random: &mut DeterministicRandom) -> Vec<String> {
    let mut corpus = vec![
        "fn main() = runtime.list()".to_string(),
        "fn main() = runtime.list(environment: \"prod\", role: none)".to_string(),
        "fn main() = runtime.inspect(runtime_id: \"runtime-a\")".to_string(),
        "fn main() = runtime.history(runtime_id: \"runtime-a\")".to_string(),
        "fn main() = runtime.refresh(runtime_id: \"runtime-a\")".to_string(),
        "fn main() = debugger.cancel(session_id: \"session-a\")".to_string(),
        "fn main() = all(left: runtime.list(), right: runtime.list(role: \"edge\"))".to_string(),
        "fn main() = seq(first: runtime.list(), next: runtime.history(runtime_id: \"runtime-a\"))"
            .to_string(),
        "fn main() = repeat(times: 3, body: runtime.list())".to_string(),
        "fn main() = repeat(times: 64, body: repeat(times: 64, body: runtime.list()))".to_string(),
        "fn main() = repeat(times: 18446744073709551616, body: runtime.list())".to_string(),
        "fn main() = bind(count: add(left: 2, right: 3), body: choose(when: ge(left: count, right: 5), then: count, otherwise: 0))".to_string(),
        "fn main() = choose(when: true, then: 1, otherwise: div(left: 1, right: 0))".to_string(),
        "fn main() = and(left: false, right: eq(left: div(left: 1, right: 0), right: 0))".to_string(),
        "fn main() = bind(count: runtime.list(), body: count)".to_string(),
        "fn main() = bind(r: runtime.list(), body: field(value: r, name: \"count\"))".to_string(),
        "fn main() = bind(r: runtime.list(), body: field(value: r, name: \"__proto__\"))".to_string(),
        "fn main() = bind(r: runtime.list(), body: add(left: field(value: r, name: \"revision\"), right: 1))".to_string(),
        "fn main() = bind(role: none, body: runtime.list(role: role))".to_string(),
        "fn main() = runtime.list(environment: concat(left: \"pro\", right: \"d\"))".to_string(),
        "fn main() = ui.focus(node_id: concat(left: \"runtime-\", right: \"a\"))".to_string(),
        "fn main() = bind(node: true, body: ui.focus(node_id: node))".to_string(),
        "fn main() = bind(role: none, body: seq(a: runtime.list(role: role), b: runtime.list()))".to_string(),
        "fn main() = repeat(times: 3, body: runtime.list(role: concat(left: \"ed\", right: \"ge\")))".to_string(),
        "fn main() = all(a: runtime.list(role: concat(left: \"ed\", right: \"ge\")), b: runtime.list())".to_string(),
        "fn main() = seq(a: runtime.list(), b: runtime.list(role: concat(left: \"bad\", right: \"\\t\")))".to_string(),
        "fn main() = concat(left: \"界面\", right: \"🙂\")".to_string(),
        String::new(),
        "\0\u{10ffff}🙂//\nfn".to_string(),
        "x".repeat(MAX_SOURCE_BYTES + 1),
    ];
    const ALPHABET: &[char] = &[
        'f',
        'n',
        'm',
        'a',
        'i',
        'r',
        'u',
        't',
        'e',
        'l',
        's',
        '.',
        '(',
        ')',
        ':',
        ',',
        '=',
        '"',
        '\\',
        '/',
        '_',
        '0',
        '9',
        ' ',
        '\n',
        '\t',
        '\0',
        'é',
        '界',
        '🙂',
        '\u{10ffff}',
    ];
    while corpus.len() < SOURCE_CASES {
        let len = random.range(513);
        let mut source = String::new();
        for _ in 0..len {
            source.push(ALPHABET[random.range(ALPHABET.len())]);
        }
        corpus.push(source);
    }
    corpus
}

fn mutate_bytes(seed: &[u8], random: &mut DeterministicRandom) -> Vec<u8> {
    let mut value = seed.to_vec();
    let edits = 1 + random.range(8);
    for _ in 0..edits {
        match random.range(4) {
            0 if !value.is_empty() => {
                let index = random.range(value.len());
                value[index] ^= random.next_u64() as u8;
            }
            1 if !value.is_empty() => {
                let index = random.range(value.len());
                value.remove(index);
            }
            2 if value.len() < 4 * 1024 => {
                let index = random.range(value.len() + 1);
                value.insert(index, random.next_u64() as u8);
            }
            _ => {
                let keep = random.range(value.len() + 1);
                value.truncate(keep);
            }
        }
    }
    value
}

struct DeterministicRandom(u64);

impl DeterministicRandom {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn range(&mut self, upper: usize) -> usize {
        usize::try_from(self.next_u64() % upper as u64).unwrap()
    }
}
