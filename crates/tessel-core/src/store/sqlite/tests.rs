use std::path::PathBuf;

use super::*;
use crate::store::{Sealer, StoreKey};

/// A database file of its own, removed at the end of the test.
struct TempFile(PathBuf);

impl TempFile {
    fn new() -> Self {
        let name: [u8; 8] = StoreKey::generate().unwrap().to_bytes()[..8]
            .try_into()
            .unwrap();
        let name: String = name.iter().map(|b| format!("{b:02x}")).collect();
        Self(std::env::temp_dir().join(format!("tessel-store-{name}.db")))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

fn record(sealer: &Sealer, partition: PartitionId, name: &[u8], body: &[u8]) -> Record {
    sealer.seal(partition, b"test", name, body).unwrap()
}

#[test]
fn a_new_database_is_at_the_current_schema_with_secure_delete() {
    let store = SqliteStore::in_memory().unwrap();
    assert_eq!(store.schema_version(), Ok(SQLITE_SCHEMA_VERSION));
    let secure_delete: i64 = store
        .connection
        .query_row("PRAGMA secure_delete", [], |row| row.get(0))
        .unwrap();
    assert_eq!(secure_delete, 1);
}

#[test]
fn records_are_written_replaced_deleted_and_listed_by_partition() {
    let key = StoreKey::generate().unwrap();
    let sealer = Sealer::new(&key).unwrap();
    let (state, history) = (
        key.state_partition().unwrap(),
        sealer.partition(b"history", b"g").unwrap(),
    );
    let mut store = SqliteStore::in_memory().unwrap();

    let (a, b, h) = (
        record(&sealer, state, b"a", b"first"),
        record(&sealer, state, b"b", b"second"),
        record(&sealer, history, b"h", b"history"),
    );
    store
        .apply(&Changes {
            put: vec![a.clone(), b.clone(), h.clone()],
            delete: vec![],
        })
        .unwrap();
    let mut listed = store.list(&state).unwrap();
    listed.sort_by_key(|r| r.id);
    let mut expected = vec![a.clone(), b.clone()];
    expected.sort_by_key(|r| r.id);
    assert_eq!(listed, expected);
    assert_eq!(store.list(&history).unwrap(), vec![h]);

    let a2 = record(&sealer, state, b"a", b"replaced");
    store
        .apply(&Changes {
            put: vec![a2.clone()],
            delete: vec![(state, b.id)],
        })
        .unwrap();
    assert_eq!(store.list(&state).unwrap(), vec![a2]);
}

#[test]
fn a_file_keeps_its_records_and_its_schema_when_reopened() {
    let file = TempFile::new();
    let key = StoreKey::generate().unwrap();
    let sealer = Sealer::new(&key).unwrap();
    let state = key.state_partition().unwrap();
    let kept = record(&sealer, state, b"a", b"kept");
    {
        let mut store = SqliteStore::open(&file.0).unwrap();
        store
            .apply(&Changes {
                put: vec![kept.clone()],
                delete: vec![],
            })
            .unwrap();
        store.set_host_value("sealed-key", b"wrapped").unwrap();
    }
    let store = SqliteStore::open(&file.0).unwrap();
    assert_eq!(store.schema_version(), Ok(SQLITE_SCHEMA_VERSION));
    assert_eq!(store.list(&state).unwrap(), vec![kept]);
    assert_eq!(
        store.host_value("sealed-key"),
        Ok(Some(b"wrapped".to_vec()))
    );
    assert_eq!(store.host_value("absent"), Ok(None));
}

#[test]
fn a_database_from_a_newer_core_is_refused() {
    let file = TempFile::new();
    {
        let store = SqliteStore::open(&file.0).unwrap();
        store
            .connection
            .pragma_update(None, "user_version", SQLITE_SCHEMA_VERSION + 1)
            .unwrap();
    }
    assert_eq!(
        SqliteStore::open(&file.0).err(),
        Some(StoreError::UnsupportedSchema(SQLITE_SCHEMA_VERSION + 1))
    );
}

#[test]
fn migrations_run_in_order_and_a_failing_one_changes_nothing() {
    let mut connection = Connection::open_in_memory().unwrap();
    let steps = [
        "CREATE TABLE one (x INTEGER);",
        "ALTER TABLE one ADD COLUMN y INTEGER; CREATE TABLE two (z INTEGER);",
    ];
    migrate(&mut connection, &steps[..1]).unwrap();
    assert_eq!(schema_version(&connection), Ok(1));
    migrate(&mut connection, &steps).unwrap();
    assert_eq!(schema_version(&connection), Ok(2));
    connection
        .execute("INSERT INTO one (x, y) VALUES (1, 2)", [])
        .unwrap();

    // The third migration fails halfway: its first statement is rolled back too.
    let broken = [
        steps[0],
        steps[1],
        "CREATE TABLE three (w INTEGER); NOT SQL;",
    ];
    assert!(matches!(
        migrate(&mut connection, &broken),
        Err(StoreError::Backend(_))
    ));
    assert_eq!(schema_version(&connection), Ok(2));
    let three: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name = 'three'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(three, 0);

    // Running them again is a no-op.
    migrate(&mut connection, &steps).unwrap();
    assert_eq!(schema_version(&connection), Ok(2));
}

#[test]
fn the_released_migrations_build_the_expected_schema() {
    let store = SqliteStore::in_memory().unwrap();
    let tables: Vec<String> = store
        .connection
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(tables, ["host", "records"]);
}
