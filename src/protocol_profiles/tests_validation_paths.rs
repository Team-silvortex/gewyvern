use std::collections::BTreeSet;
use std::fs;

use super::tests_docs_support::normalize_repo_link;

const VALIDATION_PATHS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-validation-paths.md"
);
const EXAMPLE_PATHS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-example-paths.md"
);
const COMMAND_PATHS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-command-paths.md"
);
const OPERATOR_PLAYBOOK_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-operator-playbook.md"
);
const RELEASE_HANDBOOK_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-release-handbook.md"
);
const VOLUME_GUIDE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/book/reference-protocol-volume.md"
);

fn markdown_links(content: &str) -> BTreeSet<String> {
    let mut links = BTreeSet::new();
    let mut rest = content;
    while let Some(start) = rest.find("](") {
        let candidate = &rest[start + 2..];
        let Some(end) = candidate.find(')') else {
            break;
        };
        let link = &candidate[..end];
        if let Some(relative) = normalize_repo_link(link) {
            links.insert(relative.to_string());
        }
        rest = &candidate[end + 1..];
    }
    links
}

#[test]
fn protocol_validation_paths_page_links_expected_family_hubs_and_scripts() {
    let actual = fs::read_to_string(VALIDATION_PATHS_PATH)
        .expect("protocol validation paths doc should exist");
    let links = markdown_links(&actual);
    let expected = [
        "docs/book/reference-protocol-reading-paths.md",
        "docs/book/how-to-validate-runtime-surface.md",
        "docs/script-entrypoints.md",
        "docs/book/reference-http-surface.md",
        "docs/book/reference-https-surface.md",
        "docs/book/reference-tls-surface.md",
        "docs/book/reference-dns-surface.md",
        "docs/book/reference-ssh-surface.md",
        "docs/book/reference-socks5-surface.md",
        "docs/book/reference-postgres-surface.md",
        "docs/book/reference-mysql-surface.md",
        "docs/book/reference-quic-surface.md",
        "docs/book/reference-http3-surface.md",
        "scripts/validation/high_frequency_validation.sh",
        "scripts/validation/registry_validation.sh",
        "scripts/validation/runtime_operator_validation.sh",
        "scripts/validation/three_module_stack_smoke.sh",
        "scripts/packaging/release_container_check.sh",
        "scripts/packaging/container_validation_summary.sh",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol validation paths page should link `{page}`"
        );
    }
}

#[test]
fn protocol_reading_paths_page_links_protocol_validation_paths() {
    let actual = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/book/reference-protocol-reading-paths.md"
    ))
    .expect("protocol reading paths doc should exist");
    let links = markdown_links(&actual);
    assert!(
        links.contains("docs/book/reference-protocol-validation-paths.md"),
        "protocol reading paths page should link protocol validation paths"
    );
}

#[test]
fn protocol_example_paths_page_links_expected_hubs_and_dsl_samples() {
    let actual =
        fs::read_to_string(EXAMPLE_PATHS_PATH).expect("protocol example paths doc should exist");
    let links = markdown_links(&actual);
    let expected = [
        "docs/book/reference-protocol-reading-paths.md",
        "docs/book/reference-protocol-validation-paths.md",
        "docs/book/tutorial-first-run.md",
        "docs/book/reference-http-surface.md",
        "docs/book/reference-https-surface.md",
        "docs/book/reference-tls-surface.md",
        "docs/book/reference-dns-surface.md",
        "docs/book/reference-ssh-surface.md",
        "docs/book/reference-socks5-surface.md",
        "docs/book/reference-postgres-surface.md",
        "docs/book/reference-mysql-surface.md",
        "docs/book/reference-quic-surface.md",
        "docs/book/reference-http3-surface.md",
        "docs/architecture-walkthrough-http-request.md",
        "dsl/http_request_path.gewy",
        "dsl/https_connect_process.gewy",
        "dsl/tls_client_path.gewy",
        "dsl/dns_udp_process.gewy",
        "dsl/ssh_session_path.gewy",
        "dsl/socks5_session_path.gewy",
        "dsl/postgres_simple_query_path.gewy",
        "dsl/mysql_simple_query_path.gewy",
        "dsl/quic_client_initial_path.gewy",
        "dsl/http3_request_path.gewy",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol example paths page should link `{page}`"
        );
    }
}

#[test]
fn protocol_reading_and_validation_pages_link_protocol_example_paths() {
    let reading = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/book/reference-protocol-reading-paths.md"
    ))
    .expect("protocol reading paths doc should exist");
    let validation = fs::read_to_string(VALIDATION_PATHS_PATH)
        .expect("protocol validation paths doc should exist");
    let reading_links = markdown_links(&reading);
    let validation_links = markdown_links(&validation);
    assert!(
        reading_links.contains("docs/book/reference-protocol-example-paths.md"),
        "protocol reading paths page should link protocol example paths"
    );
    assert!(
        validation_links.contains("docs/book/reference-protocol-example-paths.md"),
        "protocol validation paths page should link protocol example paths"
    );
}

#[test]
fn protocol_command_paths_page_links_expected_hubs_and_cli_guidance() {
    let actual =
        fs::read_to_string(COMMAND_PATHS_PATH).expect("protocol command paths doc should exist");
    let links = markdown_links(&actual);
    let expected = [
        "docs/book/reference-protocol-reading-paths.md",
        "docs/book/reference-protocol-validation-paths.md",
        "docs/book/reference-protocol-example-paths.md",
        "docs/cli-recipes.md",
        "docs/book/reference-http-surface.md",
        "docs/book/reference-https-surface.md",
        "docs/book/reference-tls-surface.md",
        "docs/book/reference-dns-surface.md",
        "docs/book/reference-ssh-surface.md",
        "docs/book/reference-socks5-surface.md",
        "docs/book/reference-postgres-surface.md",
        "docs/book/reference-mysql-surface.md",
        "docs/book/reference-quic-surface.md",
        "docs/book/reference-http3-surface.md",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol command paths page should link `{page}`"
        );
    }
    for command in [
        "cargo run -- --protocol http --entry request --json --summary-only",
        "cargo run -- --protocol postgres --entry query --json --summary-only",
        "cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only",
    ] {
        assert!(
            actual.contains(command),
            "protocol command paths page should mention `{command}`"
        );
    }
}

#[test]
fn example_and_validation_pages_link_protocol_command_paths() {
    let example =
        fs::read_to_string(EXAMPLE_PATHS_PATH).expect("protocol example paths doc should exist");
    let validation = fs::read_to_string(VALIDATION_PATHS_PATH)
        .expect("protocol validation paths doc should exist");
    let reading = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/book/reference-protocol-reading-paths.md"
    ))
    .expect("protocol reading paths doc should exist");
    let example_links = markdown_links(&example);
    let validation_links = markdown_links(&validation);
    let reading_links = markdown_links(&reading);
    assert!(
        example_links.contains("docs/book/reference-protocol-command-paths.md"),
        "protocol example paths page should link protocol command paths"
    );
    assert!(
        validation_links.contains("docs/book/reference-protocol-command-paths.md"),
        "protocol validation paths page should link protocol command paths"
    );
    assert!(
        reading_links.contains("docs/book/reference-protocol-command-paths.md"),
        "protocol reading paths page should link protocol command paths"
    );
}

#[test]
fn protocol_operator_playbook_links_expected_hubs_and_release_routing() {
    let actual = fs::read_to_string(OPERATOR_PLAYBOOK_PATH)
        .expect("protocol operator playbook doc should exist");
    let links = markdown_links(&actual);
    let expected = [
        "docs/book/reference-protocol-command-paths.md",
        "docs/book/reference-protocol-validation-paths.md",
        "docs/book/how-to-validate-runtime-surface.md",
        "docs/script-entrypoints.md",
        "docs/release-checklist.md",
        "docs/book/reference-http-surface.md",
        "docs/book/reference-https-surface.md",
        "docs/book/reference-tls-surface.md",
        "docs/book/reference-http3-surface.md",
        "docs/book/reference-quic-surface.md",
        "docs/book/reference-dns-surface.md",
        "docs/book/reference-ssh-surface.md",
        "docs/book/reference-socks5-surface.md",
        "docs/book/reference-postgres-surface.md",
        "docs/book/reference-mysql-surface.md",
        "docs/book/reference-redis-surface.md",
        "scripts/validation/registry_validation.sh",
        "scripts/validation/high_frequency_validation.sh",
        "scripts/validation/three_module_stack_smoke.sh",
        "scripts/packaging/release_gate.sh",
        "scripts/packaging/release_container_check.sh",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol operator playbook should link `{page}`"
        );
    }
    for command in [
        "cargo run -- --protocol redis --entry ping --json --summary-only",
        "bash scripts/packaging/release_gate.sh",
        "curl http://127.0.0.1:9100/v1/latest/analysis.json",
    ] {
        assert!(
            actual.contains(command),
            "protocol operator playbook should mention `{command}`"
        );
    }
}

#[test]
fn reading_validation_and_command_pages_link_operator_playbook() {
    let reading = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/book/reference-protocol-reading-paths.md"
    ))
    .expect("protocol reading paths doc should exist");
    let validation = fs::read_to_string(VALIDATION_PATHS_PATH)
        .expect("protocol validation paths doc should exist");
    let command =
        fs::read_to_string(COMMAND_PATHS_PATH).expect("protocol command paths doc should exist");
    let reading_links = markdown_links(&reading);
    let validation_links = markdown_links(&validation);
    let command_links = markdown_links(&command);
    assert!(
        reading_links.contains("docs/book/reference-protocol-operator-playbook.md"),
        "protocol reading paths page should link protocol operator playbook"
    );
    assert!(
        validation_links.contains("docs/book/reference-protocol-operator-playbook.md"),
        "protocol validation paths page should link protocol operator playbook"
    );
    assert!(
        command_links.contains("docs/book/reference-protocol-operator-playbook.md"),
        "protocol command paths page should link protocol operator playbook"
    );
}

#[test]
fn protocol_release_handbook_links_expected_release_and_protocol_routes() {
    let actual = fs::read_to_string(RELEASE_HANDBOOK_PATH)
        .expect("protocol release handbook doc should exist");
    let links = markdown_links(&actual);
    let expected = [
        "docs/book/reference-protocol-operator-playbook.md",
        "docs/book/reference-protocol-command-paths.md",
        "docs/release-checklist.md",
        "docs/history/v0.17.x.md",
        "docs/history/v0.17.x-midline-checklist.md",
        "docs/field-validation.md",
        "scripts/validation/registry_validation.sh",
        "scripts/validation/high_frequency_validation.sh",
        "scripts/packaging/release_container_check.sh",
        "scripts/validation/three_module_stack_smoke.sh",
        "scripts/validation/pathological_container_validation.sh",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol release handbook should link `{page}`"
        );
    }
    for command in [
        "cargo run -- --protocol http --entry request --json --summary-only",
        "cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only",
        "bash scripts/packaging/release_container_check.sh",
        "bash scripts/validation/pathological_container_validation.sh",
    ] {
        assert!(
            actual.contains(command),
            "protocol release handbook should mention `{command}`"
        );
    }
    assert!(
        actual.contains("0.17.x"),
        "protocol release handbook should mention the current 0.17.x line"
    );
}

#[test]
fn reading_command_and_operator_pages_link_release_handbook() {
    let reading = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/book/reference-protocol-reading-paths.md"
    ))
    .expect("protocol reading paths doc should exist");
    let command =
        fs::read_to_string(COMMAND_PATHS_PATH).expect("protocol command paths doc should exist");
    let operator = fs::read_to_string(OPERATOR_PLAYBOOK_PATH)
        .expect("protocol operator playbook doc should exist");
    let reading_links = markdown_links(&reading);
    let command_links = markdown_links(&command);
    let operator_links = markdown_links(&operator);
    assert!(
        reading_links.contains("docs/book/reference-protocol-release-handbook.md"),
        "protocol reading paths page should link protocol release handbook"
    );
    assert!(
        command_links.contains("docs/book/reference-protocol-release-handbook.md"),
        "protocol command paths page should link protocol release handbook"
    );
    assert!(
        operator_links.contains("docs/book/reference-protocol-release-handbook.md"),
        "protocol operator playbook should link protocol release handbook"
    );
}

#[test]
fn protocol_volume_guide_links_all_primary_protocol_reference_doors() {
    let actual =
        fs::read_to_string(VOLUME_GUIDE_PATH).expect("protocol volume guide doc should exist");
    let links = markdown_links(&actual);
    let expected = [
        "docs/book/reference-protocol-surface.md",
        "docs/book/reference-protocol-groups.md",
        "docs/book/reference-protocol-family-shelves.md",
        "docs/book/reference-protocol-reading-paths.md",
        "docs/book/reference-protocol-example-paths.md",
        "docs/book/reference-protocol-command-paths.md",
        "docs/book/reference-protocol-validation-paths.md",
        "docs/book/reference-protocol-operator-playbook.md",
        "docs/book/reference-protocol-release-handbook.md",
        "docs/book/reference-protocol-alias-index.md",
        "docs/book/reference-ir-lowering.md",
        "docs/cli-recipes.md",
        "docs/script-entrypoints.md",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    for page in expected {
        assert!(
            links.contains(&page),
            "protocol volume guide should link `{page}`"
        );
    }
}

#[test]
fn reference_and_protocol_surface_link_protocol_volume_guide() {
    let reference = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/book/reference.md"
    ))
    .expect("reference doc should exist");
    let surface = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/book/reference-protocol-surface.md"
    ))
    .expect("protocol surface doc should exist");
    let reference_links = markdown_links(&reference);
    let surface_links = markdown_links(&surface);
    assert!(
        reference_links.contains("docs/book/reference-protocol-volume.md"),
        "reference doc should link protocol volume guide"
    );
    assert!(
        surface_links.contains("docs/book/reference-protocol-volume.md"),
        "protocol surface doc should link protocol volume guide"
    );
}
