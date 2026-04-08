//! Database layer: SQLite with WAL mode and schema migrations.
//!
//! Single palace.db file replaces both ChromaDB (drawers) and SQLite KG (triples).

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// Open (or create) the palace SQLite database with WAL mode enabled.
pub fn open(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating palace directory {}", parent.display()))?;
    }

    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .with_context(|| format!("opening database {}", db_path.display()))?;

    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;

    migrate(&conn)?;
    Ok(conn)
}

/// Open an in-memory database for testing.
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

/// Run schema migrations — idempotent, safe to call on every startup.
fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- ── Drawers ──────────────────────────────────────────────────────────
        -- Replaces ChromaDB mempalace_drawers collection.
        -- embedding is a little-endian f32 byte array (384 floats = 1536 bytes).
        CREATE TABLE IF NOT EXISTS drawers (
            id          TEXT PRIMARY KEY,
            wing        TEXT NOT NULL,
            room        TEXT NOT NULL,
            content     TEXT NOT NULL,
            embedding   BLOB,
            source_file TEXT NOT NULL DEFAULT '',
            chunk_index INTEGER NOT NULL DEFAULT 0,
            added_by    TEXT NOT NULL DEFAULT 'mempalace',
            filed_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            importance  REAL NOT NULL DEFAULT 3.0
        );
        CREATE INDEX IF NOT EXISTS idx_drawers_wing ON drawers(wing);
        CREATE INDEX IF NOT EXISTS idx_drawers_room ON drawers(room);
        CREATE INDEX IF NOT EXISTS idx_drawers_source ON drawers(source_file);

        -- ── KG Entities ───────────────────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS entities (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            type        TEXT NOT NULL DEFAULT 'unknown',
            properties  TEXT NOT NULL DEFAULT '{}',
            created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        -- ── KG Triples ────────────────────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS triples (
            id              TEXT PRIMARY KEY,
            subject         TEXT NOT NULL REFERENCES entities(id),
            predicate       TEXT NOT NULL,
            object          TEXT NOT NULL REFERENCES entities(id),
            valid_from      TEXT,
            valid_to        TEXT,
            confidence      REAL NOT NULL DEFAULT 1.0,
            source_closet   TEXT,
            source_file     TEXT,
            extracted_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_triples_subject   ON triples(subject);
        CREATE INDEX IF NOT EXISTS idx_triples_object    ON triples(object);
        CREATE INDEX IF NOT EXISTS idx_triples_predicate ON triples(predicate);
        CREATE INDEX IF NOT EXISTS idx_triples_valid     ON triples(valid_from, valid_to);
        "#,
    )
    .context("running schema migrations")?;
    Ok(())
}
