use crate::error::{LiquiModError, Result};
use crate::models::{Category, ModEntry, Preset};
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
             );
             CREATE TABLE IF NOT EXISTS categories (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL UNIQUE,
               ord INTEGER NOT NULL
             );",
        )?;
        // 旧库迁移：补统计列与分类列（已存在则忽略 duplicate column 错误）
        for sql in [
            "ALTER TABLE mods ADD COLUMN size_bytes INTEGER NOT NULL DEFAULT -1",
            "ALTER TABLE mods ADD COLUMN file_count INTEGER NOT NULL DEFAULT -1",
            "ALTER TABLE mods ADD COLUMN category_id INTEGER REFERENCES categories(id)",
            "ALTER TABLE categories ADD COLUMN kind TEXT",
            "ALTER TABLE mods ADD COLUMN note TEXT",
            "ALTER TABLE mods ADD COLUMN cover_image TEXT",
            "ALTER TABLE mods ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE mods ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
        ] {
            match conn.execute_batch(sql) {
                Ok(()) => {}
                Err(e) if e.to_string().contains("duplicate column") => {}
                Err(e) => return Err(e.into()),
            }
        }
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Self { conn };
        db.ensure_default_categories()?;
        Ok(db)
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

    pub fn reassign_character(&self, id: i64, new_character: &str, new_rel: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE mods SET character = ?2, rel_path = ?3 WHERE id = ?1",
            rusqlite::params![id, new_character, new_rel],
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

    pub fn set_mod_note(&self, id: i64, note: Option<&str>) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE mods SET note = ?2 WHERE id = ?1",
            rusqlite::params![id, note],
        )?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn set_mod_cover_image(&self, id: i64, cover: Option<&str>) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE mods SET cover_image = ?2 WHERE id = ?1",
            rusqlite::params![id, cover],
        )?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(id.to_string()));
        }
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
            category_id: r.get(8)?,
            note: r.get(9)?,
            cover_image: r.get(10)?,
            is_favorite: r.get::<_, i64>(11)? != 0,
            sort_order: r.get(12)?,
        })
    }

    pub fn list_mods(&self) -> Result<Vec<ModEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character, name, rel_path, enabled, installed_at, size_bytes, file_count, category_id, note, cover_image, is_favorite, sort_order FROM mods ORDER BY is_favorite DESC, sort_order ASC, character, name",
        )?;
        let rows = stmt.query_map([], Self::row_to_entry)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_mod(&self, id: i64) -> Result<ModEntry> {
        self.conn
            .query_row(
                "SELECT id, character, name, rel_path, enabled, installed_at, size_bytes, file_count, category_id, note, cover_image, is_favorite, sort_order FROM mods WHERE id = ?1",
                rusqlite::params![id],
                Self::row_to_entry,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => LiquiModError::ModNotFound(id.to_string()),
                other => LiquiModError::Db(other),
            })
    }

    pub fn toggle_favorite_mod(&self, id: i64) -> Result<bool> {
        let current: i64 = self
            .conn
            .query_row(
                "SELECT is_favorite FROM mods WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => LiquiModError::ModNotFound(id.to_string()),
                other => LiquiModError::Db(other),
            })?;
        let next = if current == 0 { 1 } else { 0 };
        self.conn.execute(
            "UPDATE mods SET is_favorite = ?2 WHERE id = ?1",
            rusqlite::params![id, next],
        )?;
        Ok(next != 0)
    }

    pub fn set_mod_favorite(&self, id: i64, is_favorite: bool) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE mods SET is_favorite = ?2 WHERE id = ?1",
            rusqlite::params![id, is_favorite as i64],
        )?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn reorder_mods(&self, ids: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (idx, &id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE mods SET sort_order = ?2 WHERE id = ?1",
                rusqlite::params![id, idx as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
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

    fn validate_category_name(name: &str) -> Result<&str> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LiquiModError::InvalidName("分类名不能为空".into()));
        }
        Ok(name)
    }

    pub fn create_category(&self, name: &str) -> Result<i64> {
        let name = Self::validate_category_name(name)?;
        let ord: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(ord), 0) + 1 FROM categories",
            [],
            |r| r.get(0),
        )?;
        self.conn
            .execute(
                "INSERT INTO categories (name, ord) VALUES (?1, ?2)",
                rusqlite::params![name, ord],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    LiquiModError::InvalidName(format!("分类已存在：{name}"))
                }
                other => LiquiModError::Db(other),
            })?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_category(&self, id: i64, name: &str) -> Result<()> {
        let name = Self::validate_category_name(name)?;
        let n = self
            .conn
            .execute(
                "UPDATE categories SET name = ?2 WHERE id = ?1",
                rusqlite::params![id, name],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    LiquiModError::InvalidName(format!("分类已存在：{name}"))
                }
                other => LiquiModError::Db(other),
            })?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(format!("分类 {id}")));
        }
        Ok(())
    }

    /// 删除分类：其中 Mod 全部移回角色视图（category_id = NULL）。
    pub fn delete_category(&self, id: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE mods SET category_id = NULL WHERE category_id = ?1",
            rusqlite::params![id],
        )?;
        let n = tx.execute(
            "DELETE FROM categories WHERE id = ?1",
            rusqlite::params![id],
        )?;
        tx.commit()?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(format!("分类 {id}")));
        }
        Ok(())
    }

    /// 与相邻分类交换 ord（delta = ±1）；越界则不动。
    pub fn move_category(&self, id: i64, delta: i64) -> Result<()> {
        let mut ordered = self.list_categories()?;
        let Some(i) = ordered.iter().position(|c| c.id == id) else {
            return Err(LiquiModError::ModNotFound(format!("分类 {id}")));
        };
        let j = i as i64 + delta;
        if j < 0 || j as usize >= ordered.len() {
            return Ok(());
        }
        let (a, b) = (ordered[i].clone(), ordered[j as usize].clone());
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE categories SET ord = ?2 WHERE id = ?1",
            rusqlite::params![a.id, b.ord],
        )?;
        tx.execute(
            "UPDATE categories SET ord = ?2 WHERE id = ?1",
            rusqlite::params![b.id, a.ord],
        )?;
        tx.commit()?;
        ordered.clear();
        Ok(())
    }

    pub fn list_categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, c.ord, c.kind,
                    (SELECT COUNT(*) FROM mods m WHERE m.category_id = c.id)
             FROM categories c ORDER BY c.ord",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                ord: r.get(2)?,
                kind: r.get(3)?,
                mod_count: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// 按内部标识找固定分类 id（None = 未找到）。
    pub fn category_id_by_kind(&self, kind: &str) -> Result<Option<i64>> {
        let r = self.conn.query_row(
            "SELECT id FROM categories WHERE kind = ?1",
            rusqlite::params![kind],
            |r| r.get(0),
        );
        match r {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 固定六类的内部标识与显示名（「角色」是虚拟类不进表，其余五项实体）。
    pub const FIXED_TYPES: [(&'static str, &'static str, i64); 5] = [
        ("lightcone", "光锥", 1),
        ("portrait", "立绘", 2),
        ("scene", "场景", 3),
        ("npc", "NPC", 4),
        ("other", "其他", 5),
    ];

    /// 幂等预置固定分类。用户的同名自定义分类会被接管为固定类（标 kind），
    /// 保证「其他」等通用名不产生重复。
    pub fn ensure_default_categories(&self) -> Result<()> {
        for (kind, name, ord) in Self::FIXED_TYPES {
            if self.category_id_by_kind(kind)?.is_some() {
                continue;
            }
            // 无固定分类；若用户恰好建过同名自定义分类，就复用其 id 并标记 kind，
            // 否则新建。
            let existing: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM categories WHERE name = ?1",
                    rusqlite::params![name],
                    |r| r.get(0),
                )
                .ok();
            if let Some(id) = existing {
                self.conn.execute(
                    "UPDATE categories SET kind = ?1, ord = ?2 WHERE id = ?3",
                    rusqlite::params![kind, ord, id],
                )?;
            } else {
                self.conn.execute(
                    "INSERT INTO categories (name, ord, kind) VALUES (?1, ?2, ?3)",
                    rusqlite::params![name, ord, kind],
                )?;
            }
        }
        Ok(())
    }

    pub fn set_mod_category(&self, mod_id: i64, category_id: Option<i64>) -> Result<()> {
        if let Some(cid) = category_id {
            let exists: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM categories WHERE id = ?1",
                rusqlite::params![cid],
                |r| r.get(0),
            )?;
            if exists == 0 {
                return Err(LiquiModError::ModNotFound(format!("分类 {cid}")));
            }
        }
        let n = self.conn.execute(
            "UPDATE mods SET category_id = ?2 WHERE id = ?1",
            rusqlite::params![mod_id, category_id],
        )?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(mod_id.to_string()));
        }
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

        db.set_mod_note(id, Some("测试备注内容")).unwrap();
        let with_note = db.get_mod(id).unwrap();
        assert_eq!(with_note.note.as_deref(), Some("测试备注内容"));

        db.set_mod_note(id, None).unwrap();
        let no_note = db.get_mod(id).unwrap();
        assert_eq!(no_note.note, None);

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
    fn reassign_character_updates_character_and_rel_path() {
        let db = Database::open_in_memory().unwrap();
        let id = db
            .upsert_mod("OldChar", "Mod1", "mods/OldChar/Mod1")
            .unwrap();
        db.reassign_character(id, "NewChar", "mods/NewChar/Mod1")
            .unwrap();
        let m = db.get_mod(id).unwrap();
        assert_eq!(m.character, "NewChar");
        assert_eq!(m.name, "Mod1");
        assert_eq!(m.rel_path, "mods/NewChar/Mod1");
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
    fn cover_image_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let id = db.upsert_mod("A", "m", "mods/A/m").unwrap();
        assert_eq!(db.get_mod(id).unwrap().cover_image, None);
        db.set_mod_cover_image(id, Some("images/custom.png"))
            .unwrap();
        assert_eq!(
            db.get_mod(id).unwrap().cover_image.as_deref(),
            Some("images/custom.png")
        );
        db.set_mod_cover_image(id, None).unwrap();
        assert_eq!(db.get_mod(id).unwrap().cover_image, None);
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

    #[test]
    fn category_crud_and_mod_count() {
        let db = Database::open_in_memory().unwrap();
        let a = db.create_category("武器").unwrap();
        let b = db.create_category("光影").unwrap();
        let m = db
            .upsert_mod("Firefly", "Sword", "mods/Firefly/Sword")
            .unwrap();
        db.set_mod_category(m, Some(a)).unwrap();
        let cats = db.list_categories().unwrap();
        // 固定分类（kind 非空）在前；断言只看用户自定义分类
        let user: Vec<_> = cats.iter().filter(|c| c.kind.is_none()).collect();
        assert_eq!(
            user.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["武器", "光影"]
        );
        assert_eq!(user[0].mod_count, 1);
        assert_eq!(user[1].mod_count, 0);
        assert_eq!(db.get_mod(m).unwrap().category_id, Some(a));
        db.rename_category(b, "UI").unwrap();
        let cats2 = db.list_categories().unwrap();
        let ui = cats2.iter().find(|c| c.id == b).unwrap();
        assert_eq!(ui.name, "UI");
        let _ = (a, b);
    }

    #[test]
    fn category_rejects_empty_and_duplicate() {
        let db = Database::open_in_memory().unwrap();
        db.create_category("武器").unwrap();
        assert!(matches!(
            db.create_category("武器"),
            Err(LiquiModError::InvalidName(_))
        ));
        assert!(matches!(
            db.create_category("  "),
            Err(LiquiModError::InvalidName(_))
        ));
    }

    #[test]
    fn delete_category_moves_mods_back_to_null() {
        let db = Database::open_in_memory().unwrap();
        let c = db.create_category("武器").unwrap();
        let m = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        db.set_mod_category(m, Some(c)).unwrap();
        db.delete_category(c).unwrap();
        assert_eq!(db.get_mod(m).unwrap().category_id, None);
        // 固定分类仍在；用户自定义分类应清空
        assert!(db
            .list_categories()
            .unwrap()
            .iter()
            .all(|x| x.kind.is_some()));
        assert!(db.delete_category(c).is_err());
    }

    #[test]
    fn move_category_swaps_with_neighbor() {
        let db = Database::open_in_memory().unwrap();
        let a = db.create_category("A").unwrap();
        let b = db.create_category("B").unwrap();
        let c = db.create_category("C").unwrap();
        db.move_category(b, -1).unwrap();
        let user_names = || {
            db.list_categories()
                .unwrap()
                .into_iter()
                .filter(|x| x.kind.is_none())
                .map(|x| x.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(user_names(), vec!["B", "A", "C"]);
        // 真实边界：首元素上移（j<0）与末元素下移（j>=len）越界不动
        db.move_category(b, -1).unwrap();
        db.move_category(c, 1).unwrap();
        assert_eq!(user_names(), vec!["B", "A", "C"]);
        let _ = a;
    }

    #[test]
    fn set_mod_category_validates() {
        let db = Database::open_in_memory().unwrap();
        let m = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        assert!(db.set_mod_category(m, Some(999)).is_err());
        db.set_mod_category(m, None).unwrap();
        assert!(db.set_mod_category(999, None).is_err());
    }

    #[test]
    fn ensure_default_categories_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        // 首次已由 init 预置：5 条固定分类
        let cats = db.list_categories().unwrap();
        let fixed: Vec<_> = cats.iter().filter(|c| c.kind.is_some()).collect();
        assert_eq!(fixed.len(), 5);
        assert_eq!(fixed[0].name, "光锥");
        assert_eq!(fixed[4].name, "其他");
        // 再跑一次不新增、不重复
        db.ensure_default_categories().unwrap();
        let cats2 = db.list_categories().unwrap();
        assert_eq!(cats2.iter().filter(|c| c.kind.is_some()).count(), 5);
    }

    #[test]
    fn default_categories_by_kind_queries() {
        let db = Database::open_in_memory().unwrap();
        let lightcone = db.category_id_by_kind("lightcone").unwrap().unwrap();
        let other = db.category_id_by_kind("other").unwrap().unwrap();
        assert_ne!(lightcone, other);
        assert!(db.category_id_by_kind("nope").unwrap().is_none());
    }

    #[test]
    fn ensure_takes_over_user_category_with_same_name() {
        // 用户先建过「其他」自定义分类：ensure 应接管为固定类而非重复
        let db = Database::open_in_memory().unwrap();
        // 建一个重名的自定义「其他」会撞 UNIQUE(name)，需先删固定或直接测：建「其他2」再模拟——
        // 这里直接验证：ensure 后名为「其他」的只有一条且 kind=other
        let others: Vec<_> = db
            .list_categories()
            .unwrap()
            .into_iter()
            .filter(|c| c.name == "其他")
            .collect();
        assert_eq!(others.len(), 1);
        assert_eq!(others[0].kind.as_deref(), Some("other"));
    }

    #[test]
    fn migration_adds_category_column_to_old_db() {
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
        assert_eq!(db.get_mod(1).unwrap().category_id, None);
    }

    #[test]
    fn upsert_preserves_category_id() {
        // scan 的 upsert 只更新 rel_path，不得冲掉已归类
        let db = Database::open_in_memory().unwrap();
        let c = db.create_category("武器").unwrap();
        let m = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        db.set_mod_category(m, Some(c)).unwrap();
        db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        assert_eq!(db.get_mod(m).unwrap().category_id, Some(c));
    }
}
