use std::fs;
use std::path::PathBuf;

use leselang_ui::{UiDocument, UiPatch, diff, fleet_document};
use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CapabilitySet, InMemoryControlPlane, Principal, Query, QueryEnvelope,
    QueryResult, RuntimeId, RuntimeListFilter,
};
use serde::Serialize;

#[derive(Serialize)]
struct Fixture<'a> {
    schema_version: u32,
    previous: &'a UiDocument,
    patch: &'a UiPatch,
    next: &'a UiDocument,
}

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: render_conformance_fixture OUTPUT");
    let previous = fleet_document(&fleet(&[("runtime-a", "Runtime A")])).unwrap();
    let next = fleet_document(&fleet(&[
        ("runtime-a", "Runtime A"),
        ("runtime-b", "Runtime B"),
    ]))
    .unwrap();
    let patch = diff(&previous, &next).unwrap();
    let bytes = serde_json::to_vec_pretty(&Fixture {
        schema_version: 1,
        previous: &previous,
        patch: &patch,
        next: &next,
    })
    .unwrap();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(output, bytes).unwrap();
}

fn fleet(runtimes: &[(&str, &str)]) -> QueryResult {
    let mut control = InMemoryControlPlane::default();
    for (id, name) in runtimes {
        control.register_runtime(RuntimeId::new(*id).unwrap(), *name, "fixture-endpoint");
    }
    control
        .query(QueryEnvelope {
            schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
            principal: Principal {
                id: "fixture".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: Query::RuntimeList {
                filter: RuntimeListFilter::default(),
            },
        })
        .unwrap()
}
