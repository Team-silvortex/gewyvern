use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let root = PathBuf::from(std::env::var_os("GEWY_RECOVERY_PROBE_ROOT").unwrap());
    let command = std::env::args().nth(1).unwrap();
    let mode = fs::read_to_string(root.join("mode")).unwrap();
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("calls"))
            .unwrap(),
        "{command}"
    )
    .unwrap();
    if command == "protocol-capabilities" {
        if mode == "capability-failure" {
            std::process::exit(9);
        }
        println!(
            r#"{{"protocol_family":"etragon-resident-protocol","protocol_version":1,"merge_capabilities":{{"safe_automation_hints":["augmentations_and_guidance_context"],"operator_review_hints":[]}},"handoff_capabilities":{{"readiness_levels":["mergeable"]}},"context_capabilities":{{"published_contexts":["evidence_chain_enrichment"]}},"compatibility":{{"forward_compatibility_rules":[]}}}}"#
        );
        return;
    }
    assert_eq!(command, "analyze-json");
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    assert!(input.starts_with('{'));
    match mode.as_str() {
        "exit" => std::process::exit(9),
        "malformed" => println!("not-json"),
        "timeout" => std::thread::sleep(Duration::from_secs(60)),
        "overflow" => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 2 * 1024 * 1024]);
        }
        "healthy" | "capability-failure" => println!(
            r#"{{"augmentations":[{{"kind":"probe","name":"recovery_probe","summary":"external engine recovered","confidence":"advisory"}}],"evidence_chain_enrichment":{{"summary":"recovery probe context","handoff_readiness":"mergeable","gewyvern_merge_hint":"augmentations_and_guidance_context"}}}}"#
        ),
        _ => panic!("unexpected probe mode"),
    }
}
