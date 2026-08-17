use crate::error::{LiquiModError, Result};
use crate::models::ModEntry;
use rusqlite::Connection;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Database {
    conn: Connection,
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS mods (
               id INTEGER PRIMARY KEY,
               character TEXT NOT NULL,
               name TEXT NOT NULL,
               rel_path TEXT NOT NULL,
               enabled INTEGER NOT NULL DEFAULT 0,
               installed_at INTEGER NOT NULL,
               UNIQUE(character, name)
             );
             CREATE TABLE IF NOT EXISTS op_log (
               id INTEGER PRIMARY KEY,
               op TEXT NOT NULL,
               payload TEXT NOT NULL,
               finished INTEGER NOT NULL DEFAULT 0,
               created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS passwords (
               value TEXT PRIMARY KEY,
               created_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self { conn })
    }

    pub fn upsert_mod(&self, character: &str, name: &str, rel_path: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO mods (character, name, rel_path, installed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(character, name) DO UPDATE SET rel_path = excluded.rel_path",
            rusqlite::params![character, name, rel_path, now_unix()],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM mods WHERE character = ?1 AND name = ?2",
            rusqlite::params![character, name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE mods SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i64, id],
        )?;
        Ok(())
    }

    fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<ModEntry> {
        Ok(ModEntry {
            id: r.get(0)?,
            character: r.get(1)?,
            name: r.get(2)?,
            rel_path: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            installed_at: r.get(5)?,
        })
    }

    pub fn list_mods(&self) -> Result<Vec<ModEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character, name, rel_path, enabled, installed_at FROM mods ORDER BY character, name",
        )?;
        let rows = stmt.query_map([], Self::row_to_entry)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_mod(&self, id: i64) -> Result<ModEntry> {
        self.conn
            .query_row(
                "SELECT id, character, name, rel_path, enabled, installed_at FROM mods WHERE id = ?1",
                rusqlite::params![id],
                Self::row_to_entry,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => LiquiModError::ModNotFound(id.to_string()),
                other => LiquiModError::Db(other),
            })
    }

    pub fn remove_mod(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM mods WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn op_begin(&self, op: &str, payload: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO op_log (op, payload, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![op, payload, now_unix()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn op_finish(&self, op_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE op_log SET finished = 1 WHERE id = ?1",
            rusqlite::params![op_id],
        )?;
        Ok(())
    }

    pub fn remove_op(&self, op_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM op_log WHERE id = ?1", rusqlite::params![op_id])?;
        Ok(())
    }

    pub fn pending_ops(&self) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, op, payload FROM op_log WHERE finished = 0 ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn add_password(&self, value: &str) -> Result<()> {
        if value.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO passwords (value, created_at) VALUES (?1, ?2)",
            rusqlite::params![value, now_unix()],
        )?;
        Ok(())
    }

    pub fn remove_password(&self, value: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM passwords WHERE value = ?1",
            rusqlite::params![value],
        )?;
        Ok(())
    }

    pub fn list_passwords(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM passwords ORDER BY rowid")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_list_and_remove() {
        let db = Database::open_in_memory().unwrap();
        let id = db
            .upsert_mod("Firefly", "Summer", "mods/Firefly/Summer")
            .unwrap();
        let id2 = db
            .upsert_mod("Firefly", "Summer", "mods/Firefly/Summer")
            .unwrap();
        assert_eq!(id, id2);

        db.set_enabled(id, true).unwrap();
        let mods = db.list_mods().unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].character, "Firefly");
        assert_eq!(mods[0].name, "Summer");
        assert!(mods[0].enabled);

        let got = db.get_mod(id).unwrap();
        assert_eq!(got.rel_path, "mods/Firefly/Summer");

        db.remove_mod(id).unwrap();
        assert!(db.list_mods().unwrap().is_empty());
        assert!(matches!(
            db.get_mod(id),
            Err(crate::error::LiquiModError::ModNotFound(_))
        ));
    }

    #[test]
    fn op_log_lifecycle() {
        let db = Database::open_in_memory().unwrap();
        let op = db.op_begin("enable", "42").unwrap();
        let pending = db.pending_ops().unwrap();
        assert_eq!(pending, vec![(op, "enable".to_string(), "42".to_string())]);

        db.op_finish(op).unwrap();
        assert!(db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn password_book_add_list_remove() {
        let db = Database::open_in_memory().unwrap();
        db.add_password("pw-a").unwrap();
        db.add_password("pw-b").unwrap();
        db.add_password("pw-a").unwrap();
        assert_eq!(db.list_passwords().unwrap(), vec!["pw-a", "pw-b"]);
        db.remove_password("pw-a").unwrap();
        assert_eq!(db.list_passwords().unwrap(), vec!["pw-b"]);
    }

    #[test]
    fn password_empty_is_ignored() {
        let db = Database::open_in_memory().unwrap();
        db.add_password("").unwrap();
        assert!(db.list_passwords().unwrap().is_empty());
    }
}
