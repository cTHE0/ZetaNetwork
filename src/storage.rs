use rusqlite::{Connection, params};
use anyhow::Result;

use crate::crypto::KeyPair;
use crate::post::Post;

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let storage = Storage { conn };
        storage.init_tables()?;
        Ok(storage)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS keypair (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                secret_key TEXT NOT NULL,
                public_key TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS posts (
                id TEXT PRIMARY KEY,
                author_pubkey TEXT NOT NULL,
                content TEXT NOT NULL,
                signature TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_posts_author ON posts(author_pubkey);
            CREATE INDEX IF NOT EXISTS idx_posts_timestamp ON posts(timestamp);

            CREATE TABLE IF NOT EXISTS subscriptions (
                pubkey TEXT PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS peers (
                addr TEXT PRIMARY KEY,
                pubkey TEXT,
                last_seen INTEGER NOT NULL
            );"
        )?;
        Ok(())
    }

    // ========== KeyPair ==========

    pub fn save_keypair(&self, keypair: &KeyPair) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO keypair (id, secret_key, public_key) VALUES (1, ?1, ?2)",
            params![keypair.secret_hex(), keypair.public_hex()],
        )?;
        Ok(())
    }

    pub fn load_keypair(&self) -> Result<Option<KeyPair>> {
        let mut stmt = self.conn.prepare("SELECT secret_key FROM keypair WHERE id = 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let secret_hex: String = row.get(0)?;
            let keypair = KeyPair::from_secret_hex(&secret_hex)?;
            return Ok(Some(keypair));
        }
        Ok(None)
    }

    pub fn get_or_create_keypair(&self) -> Result<KeyPair> {
        if let Some(keypair) = self.load_keypair()? {
            Ok(keypair)
        } else {
            let keypair = KeyPair::generate();
            self.save_keypair(&keypair)?;
            Ok(keypair)
        }
    }

    // ========== Posts ==========

    pub fn save_post(&self, post: &Post) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO posts (id, author_pubkey, content, signature, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![post.id, post.author_pubkey, post.content, post.signature, post.timestamp],
        )?;
        Ok(())
    }

    pub fn get_posts_by_authors(&self, pubkeys: &[String], limit: usize) -> Result<Vec<Post>> {
        if pubkeys.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: Vec<String> = pubkeys.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let query = format!(
            "SELECT id, author_pubkey, content, signature, timestamp
             FROM posts WHERE author_pubkey IN ({})
             ORDER BY timestamp DESC LIMIT {}",
            placeholders.join(","),
            limit
        );
        let mut stmt = self.conn.prepare(&query)?;
        let params: Vec<&dyn rusqlite::ToSql> = pubkeys.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok(Post {
                id: row.get(0)?,
                author_pubkey: row.get(1)?,
                content: row.get(2)?,
                signature: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        let mut posts = vec![];
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }

    pub fn get_all_posts(&self, limit: usize) -> Result<Vec<Post>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, author_pubkey, content, signature, timestamp
             FROM posts ORDER BY timestamp DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(Post {
                id: row.get(0)?,
                author_pubkey: row.get(1)?,
                content: row.get(2)?,
                signature: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        let mut posts = vec![];
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }

    pub fn get_posts_since(&self, since_timestamp: u64) -> Result<Vec<Post>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, author_pubkey, content, signature, timestamp
             FROM posts WHERE timestamp > ?1 ORDER BY timestamp ASC"
        )?;
        let rows = stmt.query_map([since_timestamp], |row| {
            Ok(Post {
                id: row.get(0)?,
                author_pubkey: row.get(1)?,
                content: row.get(2)?,
                signature: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        let mut posts = vec![];
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }

    // ========== Subscriptions ==========

    pub fn add_subscription(&self, pubkey: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO subscriptions (pubkey) VALUES (?1)",
            params![pubkey],
        )?;
        Ok(())
    }

    pub fn remove_subscription(&self, pubkey: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM subscriptions WHERE pubkey = ?1",
            params![pubkey],
        )?;
        Ok(())
    }

    pub fn get_subscriptions(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT pubkey FROM subscriptions")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut subs = vec![];
        for row in rows {
            subs.push(row?);
        }
        Ok(subs)
    }

    // ========== Peers ==========

    pub fn save_peer(&self, addr: &str, pubkey: Option<&str>, last_seen: u64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO peers (addr, pubkey, last_seen) VALUES (?1, ?2, ?3)",
            params![addr, pubkey, last_seen],
        )?;
        Ok(())
    }

    pub fn get_peers(&self) -> Result<Vec<(String, Option<String>, u64)>> {
        let mut stmt = self.conn.prepare("SELECT addr, pubkey, last_seen FROM peers")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        let mut peers = vec![];
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
    }

    pub fn delete_old_peers(&self, max_age_secs: u64) -> Result<()> {
        let cutoff = crate::lib_p2p::now_secs().saturating_sub(max_age_secs);
        self.conn.execute(
            "DELETE FROM peers WHERE last_seen < ?1",
            params![cutoff],
        )?;
        Ok(())
    }

    // ========== Last Sync Timestamp (pour optimiser la synchronisation) ==========

    pub fn get_last_sync_timestamp(&self) -> Result<Option<u64>> {
        let mut stmt = self.conn.prepare(
            "SELECT MAX(timestamp) FROM posts"
        )?;
        let result: Option<u64> = stmt.query_row([], |row| row.get(0)).ok();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post::Post;
    use crate::crypto::KeyPair;

    #[test]
    fn test_storage() {
        let storage = Storage::new(":memory:").unwrap();

        // Test keypair
        let keypair = storage.get_or_create_keypair().unwrap();
        let keypair2 = storage.get_or_create_keypair().unwrap();
        assert_eq!(keypair.public_hex(), keypair2.public_hex());

        // Test subscriptions
        storage.add_subscription("abc123").unwrap();
        storage.add_subscription("def456").unwrap();
        let subs = storage.get_subscriptions().unwrap();
        assert_eq!(subs.len(), 2);

        storage.remove_subscription("abc123").unwrap();
        let subs = storage.get_subscriptions().unwrap();
        assert_eq!(subs.len(), 1);
    }
}
