//! The local database on native targets: SQLite (story 02-06).
//!
//! SQLite only stores sealed records (see [`super`]): the file reveals the number and
//! the padded size of the records, nothing else. `secure_delete` overwrites deleted
//! records, so a message deleted for everyone does not linger in free pages.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension as _, params};

use super::{Changes, PartitionId, Record, RecordId, Store, StoreError};

/// The schema migrations, in order: migration `n` brings the schema from version `n` to
/// version `n + 1`. A migration is never edited once released; a change is a new one.
const MIGRATIONS: &[&str] = &[
    // Version 1: the sealed records, and the host's own values (the sealed store key).
    "CREATE TABLE records (
        partition BLOB NOT NULL CHECK (length(partition) = 32),
        id BLOB NOT NULL CHECK (length(id) = 32),
        value BLOB NOT NULL,
        PRIMARY KEY (partition, id)
    ) WITHOUT ROWID;
    CREATE TABLE host (
        name TEXT NOT NULL PRIMARY KEY,
        value BLOB NOT NULL
    ) WITHOUT ROWID;",
];

/// Schema version this core writes.
pub const SQLITE_SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

fn backend(error: rusqlite::Error) -> StoreError {
    StoreError::Backend(error.to_string())
}

/// A SQLite database of sealed records.
#[derive(Debug)]
pub struct SqliteStore {
    connection: Connection,
}

impl SqliteStore {
    /// Opens (or creates) the database at `path` and brings its schema up to date.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::with(Connection::open(path).map_err(backend)?)
    }

    /// A database in memory, for tests.
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::with(Connection::open_in_memory().map_err(backend)?)
    }

    fn with(mut connection: Connection) -> Result<Self, StoreError> {
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 PRAGMA secure_delete = ON;
                 PRAGMA foreign_keys = ON;",
            )
            .map_err(backend)?;
        migrate(&mut connection, MIGRATIONS)?;
        Ok(Self { connection })
    }

    /// The schema version of the database.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        schema_version(&self.connection)
    }

    /// A value the host keeps next to the records (the store key sealed by the platform
    /// key store).
    pub fn host_value(&self, name: &str) -> Result<Option<Vec<u8>>, StoreError> {
        self.connection
            .query_row("SELECT value FROM host WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .optional()
            .map_err(backend)
    }

    /// Writes a host value.
    pub fn set_host_value(&mut self, name: &str, value: &[u8]) -> Result<(), StoreError> {
        self.connection
            .execute(
                "INSERT INTO host (name, value) VALUES (?1, ?2)
                 ON CONFLICT (name) DO UPDATE SET value = excluded.value",
                params![name, value],
            )
            .map(drop)
            .map_err(backend)
    }
}

impl Store for SqliteStore {
    fn apply(&mut self, changes: &Changes) -> Result<(), StoreError> {
        let transaction = self.connection.transaction().map_err(backend)?;
        {
            let mut put = transaction
                .prepare_cached(
                    "INSERT INTO records (partition, id, value) VALUES (?1, ?2, ?3)
                     ON CONFLICT (partition, id) DO UPDATE SET value = excluded.value",
                )
                .map_err(backend)?;
            for record in &changes.put {
                put.execute(params![
                    record.partition.as_bytes(),
                    record.id.as_bytes(),
                    record.value
                ])
                .map_err(backend)?;
            }
            let mut delete = transaction
                .prepare_cached("DELETE FROM records WHERE partition = ?1 AND id = ?2")
                .map_err(backend)?;
            for (partition, id) in &changes.delete {
                delete
                    .execute(params![partition.as_bytes(), id.as_bytes()])
                    .map_err(backend)?;
            }
        }
        transaction.commit().map_err(backend)
    }

    fn list(&self, partition: &PartitionId) -> Result<Vec<Record>, StoreError> {
        let mut statement = self
            .connection
            .prepare_cached("SELECT id, value FROM records WHERE partition = ?1")
            .map_err(backend)?;
        let rows = statement
            .query_map([partition.as_bytes()], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(backend)?;
        rows.map(|row| {
            let (id, value) = row.map_err(backend)?;
            Ok(Record {
                partition: *partition,
                id: RecordId::from_bytes(&id)?,
                value,
            })
        })
        .collect()
    }
}

fn schema_version(connection: &Connection) -> Result<u32, StoreError> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(backend)
}

/// Applies the migrations the database lacks, each in its own transaction with the
/// version bump: a failing migration leaves the database at the previous version.
/// Refuses a database written by a newer core rather than guessing.
fn migrate(connection: &mut Connection, migrations: &[&str]) -> Result<(), StoreError> {
    let target = u32::try_from(migrations.len()).map_err(|_| StoreError::Malformed)?;
    let current = schema_version(connection)?;
    if current > target {
        return Err(StoreError::UnsupportedSchema(current));
    }
    for (version, migration) in (current..target).zip(&migrations[current as usize..]) {
        let transaction = connection.transaction().map_err(backend)?;
        transaction.execute_batch(migration).map_err(backend)?;
        transaction
            .pragma_update(None, "user_version", version + 1)
            .map_err(backend)?;
        transaction.commit().map_err(backend)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
