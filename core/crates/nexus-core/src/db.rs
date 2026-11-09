use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct Db {
    conn: Mutex<Connection>,
}

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
     CREATE TABLE profiles (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, avatar TEXT NOT NULL, hue INTEGER NOT NULL,
        kind TEXT NOT NULL, pin_hash BLOB, pin_salt BLOB, age_limit INTEGER, daily_limit_min INTEGER,
        prefs TEXT NOT NULL DEFAULT '{}', allowed TEXT NOT NULL DEFAULT '[]', created_at INTEGER NOT NULL,
        position INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE playtime (profile_id TEXT NOT NULL, item_id TEXT NOT NULL, seconds INTEGER NOT NULL DEFAULT 0,
        last_played INTEGER, PRIMARY KEY (profile_id, item_id));
     CREATE TABLE usage (profile_id TEXT NOT NULL, day TEXT NOT NULL, seconds INTEGER NOT NULL DEFAULT 0,
        bonus INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (profile_id, day));
     CREATE TABLE installed (item_id TEXT PRIMARY KEY, ts INTEGER NOT NULL);",
];

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Db> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn memory() -> anyhow::Result<Db> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> anyhow::Result<Db> {
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, m) in MIGRATIONS.iter().enumerate().skip(v as usize) {
            conn.execute_batch(m)?;
            conn.pragma_update(None, "user_version", i as i64 + 1)?;
        }
        Ok(Db { conn: Mutex::new(conn) })
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> anyhow::Result<T> {
        let c = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        Ok(f(&c)?)
    }

    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.with(|c| c.query_row("SELECT value FROM kv WHERE key=?1", params![key], |r| r.get(0))).ok()
    }

    pub fn kv_set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.with(|c| c.execute("INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value]).map(|_| ()))
    }

    pub fn kv_del(&self, key: &str) -> anyhow::Result<()> {
        self.with(|c| c.execute("DELETE FROM kv WHERE key=?1", params![key]).map(|_| ()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kv_roundtrip() {
        let db = Db::memory().unwrap();
        assert!(db.kv_get("a").is_none());
        db.kv_set("a", "1").unwrap();
        db.kv_set("a", "2").unwrap();
        assert_eq!(db.kv_get("a").as_deref(), Some("2"));
        db.kv_del("a").unwrap();
        assert!(db.kv_get("a").is_none());
    }
}
