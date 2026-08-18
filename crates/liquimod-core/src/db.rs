use crate::error::{LiquiModError, Result};
use crate::models::{ModEntry, Preset};
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
             );
             CREATE TABLE IF NOT EXISTS presets (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL UNIQUE,
               created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS preset_entries (
               preset_id INTEGER NOT NULL REFERENCES presets(id) ON DELETE CASCADE,
               mod_id INTEGER NOT NULL,
               PRIMARY KEY (preset_id, mod_id)
             );",
        )?;
        // 旧库迁移：补统计列（已存在则忽略 duplicate column 错误）
        for col in ["size_bytes", "file_count"] {
            let sql = format!("ALTER TABLE mods ADD COLUMN {col} INTEGER NOT NULL DEFAULT -1");
            match conn.execute_batch(&sql) {
                Ok(()) => {}
                Err(e) if e.to_string().contains("duplicate column") => {}
                Err(e) => return Err(e.into()),
            }
        }
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
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

    pub fn rename_mod(&self, id: i64, new_name: &str, new_rel: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE mods SET name = ?2, rel_path = ?3 WHERE id = ?1",
            rusqlite::params![id, new_name, new_rel],
        )?;
        Ok(())
    }

    pub fn update_stats(&self, id: i64, size_bytes: i64, file_count: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE mods SET size_bytes = ?2, file_count = ?3 WHERE id = ?1",
            rusqlite::params![id, size_bytes, file_count],
        )?;
        Ok(())
    }

    pub fn name_taken(&self, character: &str, name: &str, exclude_id: i64) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM mods WHERE character = ?1 AND name = ?2 AND id != ?3",
            rusqlite::params![character, name, exclude_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<ModEntry> {
        Ok(ModEntry {
            id: r.get(0)?,
            character: r.get(1)?,
            name: r.get(2)?,
            rel_path: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            installed_at: r.get(5)?,
            size_bytes: r.get(6)?,
            file_count: r.get(7)?,
        })
    }

    pub fn list_mods(&self) -> Result<Vec<ModEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character, name, rel_path, enabled, installed_at, size_bytes, file_count FROM mods ORDER BY character, name",
        )?;
        let rows = stmt.query_map([], Self::row_to_entry)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_mod(&self, id: i64) -> Result<ModEntry> {
        self.conn
            .query_row(
                "SELECT id, character, name, rel_path, enabled, installed_at, size_bytes, file_count FROM mods WHERE id = ?1",
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

    /// 同名覆盖：条目整体替换并复用 id。条目为 id 快照——引用的 mod 被删除后条目仍保留（应用时忽略失效 id）。
    pub fn save_preset(&self, name: &str, mod_ids: &[i64]) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LiquiModError::InvalidName("预设名不能为空".into()));
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO presets (name, created_at) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET created_at = excluded.created_at",
            rusqlite::params![name, now_unix()],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM presets WHERE name = ?1",
            rusqlite::params![name],
            |r| r.get(0),
        )?;
        tx.execute(
            "DELETE FROM preset_entries WHERE preset_id = ?1",
            rusqlite::params![id],
        )?;
        for mid in mod_ids {
            tx.execute(
                "INSERT OR IGNORE INTO preset_entries (preset_id, mod_id) VALUES (?1, ?2)",
                rusqlite::params![id, mid],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn list_presets(&self) -> Result<Vec<Preset>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, created_at FROM presets ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Preset {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// 返回清单内 mod_id（按 id 升序；语义为集合，无插入顺序）。
    pub fn preset_mod_ids(&self, preset_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT mod_id FROM preset_entries WHERE preset_id = ?1 ORDER BY mod_id")?;
        let rows = stmt.query_map(rusqlite::params![preset_id], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_preset(&self, preset_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM presets WHERE id = ?1",
            rusqlite::params![preset_id],
        )?;
        Ok(())
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

    #[test]
    fn preset_roundtrip_and_overwrite() {
        let db = Database::open_in_memory().unwrap();
        let a = db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let b = db.upsert_mod("Asta", "m2", "mods/Asta/m2").unwrap();
        let id1 = db.save_preset("日常", &[a, b]).unwrap();
        assert_eq!(db.preset_mod_ids(id1).unwrap(), vec![a, b]);
        // 同名覆盖：条目整体替换，id 复用
        let id2 = db.save_preset("日常", &[b]).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(db.preset_mod_ids(id1).unwrap(), vec![b]);
        let list = db.list_presets().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "日常");
    }

    #[test]
    fn preset_delete_cascades_entries() {
        let db = Database::open_in_memory().unwrap();
        let a = db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let id = db.save_preset("x", &[a]).unwrap();
        db.delete_preset(id).unwrap();
        assert!(db.list_presets().unwrap().is_empty());
        assert!(db.preset_mod_ids(id).unwrap().is_empty());
    }

    #[test]
    fn preset_rejects_empty_name() {
        let db = Database::open_in_memory().unwrap();
        assert!(db.save_preset("  ", &[]).is_err());
    }

    #[test]
    fn migration_adds_stats_columns_to_old_db() {
        // 旧库没有 size_bytes/file_count：用裸连接建旧 schema，再 Database::open 迁移
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("old.db");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mods (
                   id INTEGER PRIMARY KEY,
                   character TEXT NOT NULL,
                   name TEXT NOT NULL,
                   rel_path TEXT NOT NULL,
                   enabled INTEGER NOT NULL DEFAULT 0,
                   installed_at INTEGER NOT NULL,
                   UNIQUE(character, name)
                 );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO mods (character, name, rel_path, installed_at) VALUES ('A','m1','mods/A/m1',1)",
                [],
            )
            .unwrap();
        }
        let db = Database::open(&path).unwrap();
        let m = db.get_mod(1).unwrap();
        assert_eq!(m.size_bytes, -1); // 旧行默认 -1（未统计）
        assert_eq!(m.file_count, -1);
    }

    #[test]
    fn rename_mod_updates_name_and_rel_path() {
        let db = Database::open_in_memory().unwrap();
        let id = db.upsert_mod("A", "old", "mods/A/old").unwrap();
        db.rename_mod(id, "new", "mods/A/new").unwrap();
        let m = db.get_mod(id).unwrap();
        assert_eq!(m.name, "new");
        assert_eq!(m.rel_path, "mods/A/new");
        assert!(!m.enabled && m.installed_at > 0);
    }

    #[test]
    fn update_stats_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let id = db.upsert_mod("A", "m", "mods/A/m").unwrap();
        db.update_stats(id, 12345, 7).unwrap();
        let m = db.get_mod(id).unwrap();
        assert_eq!((m.size_bytes, m.file_count), (12345, 7));
    }

    #[test]
    fn name_taken_excludes_self() {
        let db = Database::open_in_memory().unwrap();
        let id = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        db.upsert_mod("A", "m2", "mods/A/m2").unwrap();
        assert!(db.name_taken("A", "m2", id).unwrap());
        assert!(!db.name_taken("A", "m1", id).unwrap()); // 自己不算占用
        assert!(!db.name_taken("B", "m2", id).unwrap()); // 跨角色不冲突
    }
}
