#[test]
fn linked_sqlite_includes_the_wal_page_size_security_fix() {
    assert!(
        rusqlite::version_number() >= 3_054_000,
        "RUSTSEC-2026-0459: linked SQLite {} predates the WAL page-size fix",
        rusqlite::version()
    );
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    let version: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, rusqlite::version());
}
