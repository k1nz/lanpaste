use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::types::{
    is_legacy_default_shortcut, AppSettings, HistoryEntry, PasteType, Preview,
    DEFAULT_AUTO_SYNC_MAX_BYTES, DEFAULT_CLEANUP_MAX_BYTES, DEFAULT_CLEANUP_MAX_ITEMS,
    DEFAULT_SHORTCUT,
};

pub struct Store {
    conn: Connection,
    pub blob_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredItem {
    pub id: String,
    pub pasteboard_id: String,
    pub sort_order: i64,
    pub item_type: String,
    pub text_content: Option<String>,
    pub html_content: Option<String>,
    pub rtf_b64: Option<String>,
    pub url: Option<String>,
    pub color: Option<String>,
    pub blob_hash: Option<String>,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub download_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredPasteboard {
    pub id: String,
    pub copied_at: i64,
    pub source_device_id: Option<String>,
    pub source_device_name: String,
    pub primary_type: String,
    pub title: String,
    pub content_hash: String,
    pub total_bytes: u64,
    pub needs_file_download: bool,
    pub file_download_state: String,
    pub download_token: Option<String>,
    pub source_host: Option<String>,
    pub source_port: Option<i64>,
    pub preview_json: String,
}

#[derive(Debug, Clone)]
pub struct StoredDevice {
    pub instance_id: String,
    pub name: String,
    pub note: String,
    pub fingerprint: String,
    pub public_key: String,
    pub cert_der: String,
    pub allow_send: bool,
    pub allow_receive: bool,
    pub auto_write_clipboard: bool,
    pub trust_broken: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl Store {
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_dir).map_err(|e| format!("data dir: {e}"))?;
        let blob_dir = data_dir.join("blobs");
        fs::create_dir_all(&blob_dir).map_err(|e| format!("blob dir: {e}"))?;
        let db_path = data_dir.join("lanpaste.db");
        let conn = Connection::open(db_path).map_err(|e| format!("sqlite: {e}"))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| format!("wal: {e}"))?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| format!("fk: {e}"))?;
        let store = Self { conn, blob_dir };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS pasteboards (
                  id TEXT PRIMARY KEY,
                  copied_at INTEGER NOT NULL,
                  source_device_id TEXT,
                  source_device_name TEXT NOT NULL,
                  primary_type TEXT NOT NULL,
                  title TEXT NOT NULL,
                  content_hash TEXT NOT NULL,
                  total_bytes INTEGER NOT NULL,
                  needs_file_download INTEGER NOT NULL DEFAULT 0,
                  file_download_state TEXT NOT NULL DEFAULT 'idle',
                  download_token TEXT,
                  source_host TEXT,
                  source_port INTEGER,
                  preview_json TEXT NOT NULL DEFAULT '{}'
                );
                CREATE TABLE IF NOT EXISTS items (
                  id TEXT PRIMARY KEY,
                  pasteboard_id TEXT NOT NULL,
                  sort_order INTEGER NOT NULL,
                  item_type TEXT NOT NULL,
                  text_content TEXT,
                  html_content TEXT,
                  rtf_b64 TEXT,
                  url TEXT,
                  color TEXT,
                  blob_hash TEXT,
                  file_name TEXT,
                  file_size INTEGER,
                  width INTEGER,
                  height INTEGER,
                  download_token TEXT,
                  FOREIGN KEY (pasteboard_id) REFERENCES pasteboards(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_pb_copied ON pasteboards(copied_at DESC);
                CREATE INDEX IF NOT EXISTS idx_items_pb ON items(pasteboard_id);
                CREATE TABLE IF NOT EXISTS devices (
                  instance_id TEXT PRIMARY KEY,
                  name TEXT NOT NULL,
                  note TEXT NOT NULL DEFAULT '',
                  fingerprint TEXT NOT NULL,
                  public_key TEXT NOT NULL,
                  cert_der TEXT NOT NULL,
                  allow_send INTEGER NOT NULL DEFAULT 1,
                  allow_receive INTEGER NOT NULL DEFAULT 1,
                  auto_write_clipboard INTEGER NOT NULL DEFAULT 1,
                  trust_broken INTEGER NOT NULL DEFAULT 0,
                  host TEXT,
                  port INTEGER
                );
                CREATE TABLE IF NOT EXISTS pending_sync (
                  pasteboard_id TEXT NOT NULL,
                  device_id TEXT NOT NULL,
                  created_at INTEGER NOT NULL,
                  PRIMARY KEY (pasteboard_id, device_id)
                );
                CREATE TABLE IF NOT EXISTS settings (
                  key TEXT PRIMARY KEY,
                  value TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS blobs (
                  hash TEXT PRIMARY KEY,
                  size INTEGER NOT NULL,
                  refcount INTEGER NOT NULL DEFAULT 0
                );
                "#,
            )
            .map_err(|e| format!("migrate: {e}"))?;
        Ok(())
    }

    pub fn last_content_hash(&self) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT content_hash FROM pasteboards ORDER BY copied_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("last hash: {e}"))
    }

    pub fn insert_pasteboard(
        &mut self,
        pb: &StoredPasteboard,
        items: &[StoredItem],
    ) -> Result<(), String> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("tx: {e}"))?;
        tx.execute(
            "INSERT INTO pasteboards (id, copied_at, source_device_id, source_device_name, primary_type, title, content_hash, total_bytes, needs_file_download, file_download_state, download_token, source_host, source_port, preview_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                pb.id,
                pb.copied_at,
                pb.source_device_id,
                pb.source_device_name,
                pb.primary_type,
                pb.title,
                pb.content_hash,
                pb.total_bytes as i64,
                pb.needs_file_download as i64,
                pb.file_download_state,
                pb.download_token,
                pb.source_host,
                pb.source_port,
                pb.preview_json,
            ],
        )
        .map_err(|e| format!("insert pasteboard: {e}"))?;
        for item in items {
            tx.execute(
                "INSERT INTO items (id, pasteboard_id, sort_order, item_type, text_content, html_content, rtf_b64, url, color, blob_hash, file_name, file_size, width, height, download_token)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                params![
                    item.id,
                    item.pasteboard_id,
                    item.sort_order,
                    item.item_type,
                    item.text_content,
                    item.html_content,
                    item.rtf_b64,
                    item.url,
                    item.color,
                    item.blob_hash,
                    item.file_name,
                    item.file_size.map(|s| s as i64),
                    item.width.map(|w| w as i64),
                    item.height.map(|h| h as i64),
                    item.download_token,
                ],
            )
            .map_err(|e| format!("insert item: {e}"))?;
            if let Some(hash) = &item.blob_hash {
                tx.execute(
                    "UPDATE blobs SET refcount = refcount + 1 WHERE hash = ?1",
                    params![hash],
                )
                .map_err(|e| format!("blob ref: {e}"))?;
            }
        }
        tx.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(())
    }

    pub fn pasteboard_exists(&self, id: &str) -> Result<bool, String> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pasteboards WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("exists: {e}"))?;
        Ok(n > 0)
    }

    pub fn get_pasteboard(&self, id: &str) -> Result<Option<StoredPasteboard>, String> {
        self.conn
            .query_row(
                "SELECT id, copied_at, source_device_id, source_device_name, primary_type, title, content_hash, total_bytes, needs_file_download, file_download_state, download_token, source_host, source_port, preview_json
                 FROM pasteboards WHERE id = ?1",
                params![id],
                row_to_pb,
            )
            .optional()
            .map_err(|e| format!("get pasteboard: {e}"))
    }

    pub fn get_items(&self, pasteboard_id: &str) -> Result<Vec<StoredItem>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, pasteboard_id, sort_order, item_type, text_content, html_content, rtf_b64, url, color, blob_hash, file_name, file_size, width, height, download_token
                 FROM items WHERE pasteboard_id = ?1 ORDER BY sort_order",
            )
            .map_err(|e| format!("items: {e}"))?;
        let rows = stmt
            .query_map(params![pasteboard_id], row_to_item)
            .map_err(|e| format!("items query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("item row: {e}"))?);
        }
        Ok(out)
    }

    pub fn get_item(&self, id: &str) -> Result<Option<StoredItem>, String> {
        self.conn
            .query_row(
                "SELECT id, pasteboard_id, sort_order, item_type, text_content, html_content, rtf_b64, url, color, blob_hash, file_name, file_size, width, height, download_token
                 FROM items WHERE id = ?1",
                params![id],
                row_to_item,
            )
            .optional()
            .map_err(|e| format!("get item: {e}"))
    }

    /// Resolve a file item from either a pasteboard id or an item id.
    /// Peers download with `GET /files/{item.id}`.
    pub fn find_file_for_download(&self, id: &str) -> Result<Option<StoredItem>, String> {
        if self.get_pasteboard(id)?.is_some() {
            if let Some(it) = self.get_items(id)?.into_iter().find(|i| i.item_type == "file") {
                return Ok(Some(it));
            }
        }
        if let Some(it) = self.get_item(id)? {
            if it.item_type == "file" {
                return Ok(Some(it));
            }
            if let Some(file) = self
                .get_items(&it.pasteboard_id)?
                .into_iter()
                .find(|i| i.item_type == "file")
            {
                return Ok(Some(file));
            }
        }
        Ok(None)
    }

    pub fn list_history(
        &self,
        query: Option<&str>,
        type_filter: Option<&str>,
        blob_dir: &Path,
    ) -> Result<Vec<HistoryEntry>, String> {
        let mut sql = String::from(
            "SELECT DISTINCT p.id, p.copied_at, p.source_device_id, p.source_device_name, p.primary_type, p.title, p.content_hash, p.total_bytes, p.needs_file_download, p.file_download_state, p.download_token, p.source_host, p.source_port, p.preview_json
             FROM pasteboards p LEFT JOIN items i ON i.pasteboard_id = p.id WHERE 1=1",
        );
        let mut args: Vec<String> = Vec::new();
        if let Some(tf) = type_filter {
            if tf != "all" {
                sql.push_str(" AND p.primary_type = ?");
                args.push(tf.to_string());
            }
        }
        if let Some(q) = query {
            let q = q.trim();
            if !q.is_empty() {
                sql.push_str(" AND (p.title LIKE ? OR IFNULL(i.text_content,'') LIKE ? OR IFNULL(i.file_name,'') LIKE ? OR IFNULL(i.url,'') LIKE ?)");
                let like = format!("%{q}%");
                args.push(like.clone());
                args.push(like.clone());
                args.push(like.clone());
                args.push(like);
            }
        }
        sql.push_str(" ORDER BY p.copied_at DESC");

        let mut stmt = self.conn.prepare(&sql).map_err(|e| format!("list: {e}"))?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = args
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), row_to_pb)
            .map_err(|e| format!("list query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            let pb = row.map_err(|e| format!("list row: {e}"))?;
            out.push(pb_to_entry(&pb, blob_dir)?);
        }
        Ok(out)
    }

    pub fn get_entry(&self, id: &str) -> Result<Option<HistoryEntry>, String> {
        match self.get_pasteboard(id)? {
            Some(pb) => Ok(Some(pb_to_entry(&pb, &self.blob_dir)?)),
            None => Ok(None),
        }
    }

    pub fn delete_pasteboard(&mut self, id: &str) -> Result<(), String> {
        let items = self.get_items(id)?;
        let tx = self.conn.transaction().map_err(|e| format!("tx: {e}"))?;
        tx.execute("DELETE FROM pending_sync WHERE pasteboard_id = ?1", params![id])
            .map_err(|e| format!("del queue: {e}"))?;
        tx.execute("DELETE FROM items WHERE pasteboard_id = ?1", params![id])
            .map_err(|e| format!("del items: {e}"))?;
        tx.execute("DELETE FROM pasteboards WHERE id = ?1", params![id])
            .map_err(|e| format!("del pb: {e}"))?;
        for item in items {
            if let Some(hash) = item.blob_hash {
                tx.execute(
                    "UPDATE blobs SET refcount = MAX(refcount - 1, 0) WHERE hash = ?1",
                    params![hash],
                )
                .map_err(|e| format!("dec ref: {e}"))?;
            }
        }
        tx.commit().map_err(|e| format!("commit: {e}"))?;
        self.gc_blobs()?;
        Ok(())
    }

    pub fn update_download_state(&self, id: &str, state: &str, needs: bool) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE pasteboards SET file_download_state = ?1, needs_file_download = ?2 WHERE id = ?3",
                params![state, needs as i64, id],
            )
            .map_err(|e| format!("download state: {e}"))?;
        Ok(())
    }

    pub fn attach_blob_to_file_item(
        &mut self,
        pasteboard_id: &str,
        blob_hash: &str,
        file_size: u64,
        preview_json: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE items SET blob_hash = ?1, file_size = ?2 WHERE pasteboard_id = ?3 AND item_type = 'file'",
                params![blob_hash, file_size as i64, pasteboard_id],
            )
            .map_err(|e| format!("attach blob: {e}"))?;
        self.conn
            .execute(
                "UPDATE blobs SET refcount = refcount + 1 WHERE hash = ?1",
                params![blob_hash],
            )
            .map_err(|e| format!("blob ref: {e}"))?;
        self.conn
            .execute(
                "UPDATE pasteboards SET needs_file_download = 0, file_download_state = 'idle', preview_json = ?1 WHERE id = ?2",
                params![preview_json, pasteboard_id],
            )
            .map_err(|e| format!("pb update: {e}"))?;
        Ok(())
    }

    pub fn list_oldest(&self) -> Result<Vec<StoredPasteboard>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, copied_at, source_device_id, source_device_name, primary_type, title, content_hash, total_bytes, needs_file_download, file_download_state, download_token, source_host, source_port, preview_json
                 FROM pasteboards ORDER BY copied_at ASC",
            )
            .map_err(|e| format!("oldest: {e}"))?;
        let rows = stmt
            .query_map([], row_to_pb)
            .map_err(|e| format!("oldest query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("oldest row: {e}"))?);
        }
        Ok(out)
    }

    pub fn stats(&self) -> Result<(u64, u64), String> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM pasteboards", [], |r| r.get(0))
            .map_err(|e| format!("count: {e}"))?;
        let bytes: i64 = self
            .conn
            .query_row(
                "SELECT IFNULL(SUM(size), 0) FROM blobs WHERE refcount > 0",
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("bytes: {e}"))?;
        Ok((count as u64, bytes as u64))
    }

    pub fn pasteboard_blob_hashes(&self, id: &str) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT blob_hash FROM items WHERE pasteboard_id = ?1 AND blob_hash IS NOT NULL")
            .map_err(|e| format!("hashes: {e}"))?;
        let rows = stmt
            .query_map(params![id], |r| r.get::<_, String>(0))
            .map_err(|e| format!("hashes query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("hash row: {e}"))?);
        }
        Ok(out)
    }

    fn gc_blobs(&self) -> Result<(), String> {
        let mut stmt = self
            .conn
            .prepare("SELECT hash FROM blobs WHERE refcount <= 0")
            .map_err(|e| format!("gc: {e}"))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| format!("gc query: {e}"))?;
        let mut dead = Vec::new();
        for row in rows {
            dead.push(row.map_err(|e| format!("gc row: {e}"))?);
        }
        for hash in dead {
            let path = self.blob_dir.join(&hash);
            let _ = fs::remove_file(path);
            let _ = fs::remove_dir_all(self.blob_dir.join("named").join(&hash));
            self.conn
                .execute("DELETE FROM blobs WHERE hash = ?1", params![hash])
                .map_err(|e| format!("gc del: {e}"))?;
        }
        Ok(())
    }

    pub fn put_blob(&self, bytes: &[u8]) -> Result<(String, u64), String> {
        let hash = hex::encode(Sha256::digest(bytes));
        self.ensure_blob_row(&hash, bytes.len() as u64, Some(bytes), None)?;
        Ok((hash, bytes.len() as u64))
    }

    pub fn ingest_file(&self, src: &Path) -> Result<(String, u64), String> {
        let mut file = fs::File::open(src).map_err(|e| format!("open file: {e}"))?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536];
        let mut size = 0u64;
        loop {
            let n = file.read(&mut buf).map_err(|e| format!("read file: {e}"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            size += n as u64;
        }
        let hash = hex::encode(hasher.finalize());
        self.ensure_blob_row(&hash, size, None, Some(src))?;
        Ok((hash, size))
    }

    fn ensure_blob_row(
        &self,
        hash: &str,
        size: u64,
        bytes: Option<&[u8]>,
        src: Option<&Path>,
    ) -> Result<(), String> {
        let dest = self.blob_dir.join(hash);
        if !dest.exists() {
            if let Some(bytes) = bytes {
                let mut f = fs::File::create(&dest).map_err(|e| format!("create blob: {e}"))?;
                f.write_all(bytes).map_err(|e| format!("write blob: {e}"))?;
            } else if let Some(src) = src {
                if fs::hard_link(src, &dest).is_err() {
                    fs::copy(src, &dest).map_err(|e| format!("copy blob: {e}"))?;
                }
            }
        }
        let exists: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM blobs WHERE hash = ?1",
                params![hash],
                |r| r.get(0),
            )
            .map_err(|e| format!("blob exists: {e}"))?;
        if exists == 0 {
            self.conn
                .execute(
                    "INSERT INTO blobs (hash, size, refcount) VALUES (?1, ?2, 0)",
                    params![hash, size as i64],
                )
                .map_err(|e| format!("insert blob: {e}"))?;
        }
        Ok(())
    }

    pub fn blob_path(&self, hash: &str) -> PathBuf {
        self.blob_dir.join(hash)
    }

    /// Clipboard and Finder need a real filename + extension. Blobs stay
    /// content-addressed; this hardlinks (or copies) to `named/{hash}/{name}`.
    pub fn named_blob_path(&self, hash: &str, file_name: Option<&str>) -> Result<PathBuf, String> {
        let src = self.blob_path(hash);
        if !src.exists() {
            return Err("source_file_gone".into());
        }
        let name = sanitize_file_name(file_name);
        let dir = self.blob_dir.join("named").join(hash);
        fs::create_dir_all(&dir).map_err(|e| format!("named dir: {e}"))?;
        let dest = dir.join(&name);
        if dest.exists() {
            return Ok(dest);
        }
        match fs::hard_link(&src, &dest) {
            Ok(()) => Ok(dest),
            Err(_) if dest.exists() => Ok(dest),
            Err(_) => {
                fs::copy(&src, &dest).map_err(|e| format!("named copy: {e}"))?;
                Ok(dest)
            }
        }
    }

    pub fn read_blob(&self, hash: &str) -> Result<Vec<u8>, String> {
        fs::read(self.blob_path(hash)).map_err(|e| format!("read blob: {e}"))
    }

    pub fn upsert_device(&self, d: &StoredDevice) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO devices (instance_id, name, note, fingerprint, public_key, cert_der, allow_send, allow_receive, auto_write_clipboard, trust_broken, host, port)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(instance_id) DO UPDATE SET
                   name=excluded.name,
                   fingerprint=excluded.fingerprint,
                   public_key=excluded.public_key,
                   cert_der=excluded.cert_der,
                   trust_broken=excluded.trust_broken,
                   host=COALESCE(excluded.host, devices.host),
                   port=COALESCE(excluded.port, devices.port)",
                params![
                    d.instance_id,
                    d.name,
                    d.note,
                    d.fingerprint,
                    d.public_key,
                    d.cert_der,
                    d.allow_send as i64,
                    d.allow_receive as i64,
                    d.auto_write_clipboard as i64,
                    d.trust_broken as i64,
                    d.host,
                    d.port.map(|p| p as i64),
                ],
            )
            .map_err(|e| format!("upsert device: {e}"))?;
        Ok(())
    }

    pub fn get_device(&self, instance_id: &str) -> Result<Option<StoredDevice>, String> {
        self.conn
            .query_row(
                "SELECT instance_id, name, note, fingerprint, public_key, cert_der, allow_send, allow_receive, auto_write_clipboard, trust_broken, host, port
                 FROM devices WHERE instance_id = ?1",
                params![instance_id],
                row_to_device,
            )
            .optional()
            .map_err(|e| format!("get device: {e}"))
    }

    pub fn list_devices(&self) -> Result<Vec<StoredDevice>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT instance_id, name, note, fingerprint, public_key, cert_der, allow_send, allow_receive, auto_write_clipboard, trust_broken, host, port
                 FROM devices",
            )
            .map_err(|e| format!("list devices: {e}"))?;
        let rows = stmt
            .query_map([], row_to_device)
            .map_err(|e| format!("devices query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("device row: {e}"))?);
        }
        Ok(out)
    }

    pub fn update_device_note(&self, instance_id: &str, note: &str) -> Result<(), String> {
        let n = self
            .conn
            .execute(
                "UPDATE devices SET note = ?1 WHERE instance_id = ?2",
                params![note, instance_id],
            )
            .map_err(|e| format!("note: {e}"))?;
        if n == 0 {
            return Err("device_removed".into());
        }
        Ok(())
    }

    pub fn update_device_flags(
        &self,
        instance_id: &str,
        allow_send: bool,
        allow_receive: bool,
        auto_write: bool,
    ) -> Result<(), String> {
        let n = self
            .conn
            .execute(
                "UPDATE devices SET allow_send=?1, allow_receive=?2, auto_write_clipboard=?3 WHERE instance_id=?4",
                params![allow_send as i64, allow_receive as i64, auto_write as i64, instance_id],
            )
            .map_err(|e| format!("flags: {e}"))?;
        if n == 0 {
            return Err("device_removed".into());
        }
        Ok(())
    }

    pub fn update_device_addr(&self, instance_id: &str, host: &str, port: u16) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE devices SET host=?1, port=?2 WHERE instance_id=?3",
                params![host, port as i64, instance_id],
            )
            .map_err(|e| format!("addr: {e}"))?;
        Ok(())
    }

    pub fn mark_trust_broken(&self, instance_id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE devices SET trust_broken=1 WHERE instance_id=?1",
                params![instance_id],
            )
            .map_err(|e| format!("trust: {e}"))?;
        Ok(())
    }

    pub fn remove_device(&self, instance_id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM pending_sync WHERE device_id=?1",
                params![instance_id],
            )
            .map_err(|e| format!("del pending: {e}"))?;
        self.conn
            .execute(
                "DELETE FROM devices WHERE instance_id=?1",
                params![instance_id],
            )
            .map_err(|e| format!("del device: {e}"))?;
        Ok(())
    }

    pub fn enqueue_sync(&self, pasteboard_id: &str, device_id: &str) -> Result<(), String> {
        let now = now_ms();
        self.conn
            .execute(
                "INSERT OR IGNORE INTO pending_sync (pasteboard_id, device_id, created_at) VALUES (?1,?2,?3)",
                params![pasteboard_id, device_id, now],
            )
            .map_err(|e| format!("enqueue: {e}"))?;
        Ok(())
    }

    pub fn dequeue_sync(&self, pasteboard_id: &str, device_id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM pending_sync WHERE pasteboard_id=?1 AND device_id=?2",
                params![pasteboard_id, device_id],
            )
            .map_err(|e| format!("dequeue: {e}"))?;
        Ok(())
    }

    pub fn list_pending_sync(&self) -> Result<Vec<(String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT pasteboard_id, device_id FROM pending_sync ORDER BY created_at")
            .map_err(|e| format!("pending: {e}"))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| format!("pending query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("pending row: {e}"))?);
        }
        Ok(out)
    }

    pub fn load_settings(&self) -> Result<AppSettings, String> {
        let mut s = AppSettings::default();
        if let Some(v) = self.get_setting("autoSyncMaxBytes")? {
            if let Ok(n) = v.parse() {
                s.auto_sync_max_bytes = n;
            }
        }
        if let Some(v) = self.get_setting("cleanupMaxItems")? {
            if let Ok(n) = v.parse() {
                s.cleanup_max_items = n;
            }
        }
        if let Some(v) = self.get_setting("cleanupMaxBytes")? {
            if let Ok(n) = v.parse() {
                s.cleanup_max_bytes = n;
            }
        }
        if let Some(v) = self.get_setting("cleanupMaxAgeDays")? {
            if v.is_empty() || v == "null" {
                s.cleanup_max_age_days = None;
            } else if let Ok(n) = v.parse() {
                s.cleanup_max_age_days = Some(n);
            }
        }
        if let Some(v) = self.get_setting("overlayShortcut")? {
            if !v.is_empty() && !is_legacy_default_shortcut(&v) {
                s.overlay_shortcut = v;
            }
        }
        let _ = (
            DEFAULT_AUTO_SYNC_MAX_BYTES,
            DEFAULT_CLEANUP_MAX_ITEMS,
            DEFAULT_CLEANUP_MAX_BYTES,
            DEFAULT_SHORTCUT,
        );
        Ok(s)
    }

    fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| format!("setting: {e}"))
    }

    pub fn save_settings(&self, s: &AppSettings) -> Result<(), String> {
        self.put_setting("autoSyncMaxBytes", &s.auto_sync_max_bytes.to_string())?;
        self.put_setting("cleanupMaxItems", &s.cleanup_max_items.to_string())?;
        self.put_setting("cleanupMaxBytes", &s.cleanup_max_bytes.to_string())?;
        let age = s
            .cleanup_max_age_days
            .map(|d| d.to_string())
            .unwrap_or_default();
        self.put_setting("cleanupMaxAgeDays", &age)?;
        self.put_setting("overlayShortcut", &s.overlay_shortcut)?;
        Ok(())
    }

    fn put_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, value],
            )
            .map_err(|e| format!("put setting: {e}"))?;
        Ok(())
    }
}

fn row_to_pb(r: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPasteboard> {
    Ok(StoredPasteboard {
        id: r.get(0)?,
        copied_at: r.get(1)?,
        source_device_id: r.get(2)?,
        source_device_name: r.get(3)?,
        primary_type: r.get(4)?,
        title: r.get(5)?,
        content_hash: r.get(6)?,
        total_bytes: r.get::<_, i64>(7)? as u64,
        needs_file_download: r.get::<_, i64>(8)? != 0,
        file_download_state: r.get(9)?,
        download_token: r.get(10)?,
        source_host: r.get(11)?,
        source_port: r.get(12)?,
        preview_json: r.get(13)?,
    })
}

fn row_to_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<StoredItem> {
    Ok(StoredItem {
        id: r.get(0)?,
        pasteboard_id: r.get(1)?,
        sort_order: r.get(2)?,
        item_type: r.get(3)?,
        text_content: r.get(4)?,
        html_content: r.get(5)?,
        rtf_b64: r.get(6)?,
        url: r.get(7)?,
        color: r.get(8)?,
        blob_hash: r.get(9)?,
        file_name: r.get(10)?,
        file_size: r.get::<_, Option<i64>>(11)?.map(|n| n as u64),
        width: r.get::<_, Option<i64>>(12)?.map(|n| n as u32),
        height: r.get::<_, Option<i64>>(13)?.map(|n| n as u32),
        download_token: r.get(14)?,
    })
}

fn row_to_device(r: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDevice> {
    Ok(StoredDevice {
        instance_id: r.get(0)?,
        name: r.get(1)?,
        note: r.get(2)?,
        fingerprint: r.get(3)?,
        public_key: r.get(4)?,
        cert_der: r.get(5)?,
        allow_send: r.get::<_, i64>(6)? != 0,
        allow_receive: r.get::<_, i64>(7)? != 0,
        auto_write_clipboard: r.get::<_, i64>(8)? != 0,
        trust_broken: r.get::<_, i64>(9)? != 0,
        host: r.get(10)?,
        port: r.get::<_, Option<i64>>(11)?.map(|n| n as u16),
    })
}

fn pb_to_entry(pb: &StoredPasteboard, blob_dir: &Path) -> Result<HistoryEntry, String> {
    let preview: Preview =
        serde_json::from_str(&pb.preview_json).unwrap_or_default();
    if let Some(name) = preview.file_name.clone() {
        if preview.path.is_none() {
            // Best-effort local blob path when we already have a hash in preview.path later.
            let _ = name;
            let _ = blob_dir;
        }
    }
    Ok(HistoryEntry {
        id: pb.id.clone(),
        copied_at: pb.copied_at,
        source_device_id: pb.source_device_id.clone(),
        source_device_name: pb.source_device_name.clone(),
        primary_type: PasteType::parse(&pb.primary_type).unwrap_or(PasteType::Text),
        title: pb.title.clone(),
        preview,
        needs_file_download: pb.needs_file_download,
        file_download_state: pb.file_download_state.clone(),
    })
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn sanitize_file_name(name: Option<&str>) -> String {
    let raw = name.unwrap_or("").trim();
    let base = Path::new(raw)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| raw.to_string());
    let mut out = String::new();
    for c in base.chars() {
        if c.is_control()
            || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0')
        {
            continue;
        }
        out.push(c);
        if out.chars().count() >= 255 {
            break;
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() || out == "." || out == ".." {
        "file".into()
    } else {
        out
    }
}

pub fn content_hash_for(parts: &[(String, Vec<u8>)]) -> String {
    let mut hasher = Sha256::new();
    for (ty, bytes) in parts {
        hasher.update(ty.as_bytes());
        hasher.update([0u8]);
        hasher.update(bytes);
        hasher.update([0xffu8]);
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_hash_same_content() {
        let a = content_hash_for(&[("text".into(), b"hello".to_vec())]);
        let b = content_hash_for(&[("text".into(), b"hello".to_vec())]);
        let c = content_hash_for(&[("text".into(), b"world".to_vec())]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    fn sample_pb(id: &str) -> StoredPasteboard {
        StoredPasteboard {
            id: id.into(),
            copied_at: 1,
            source_device_id: Some("dev".into()),
            source_device_name: "Dev".into(),
            primary_type: "file".into(),
            title: "a.bin".into(),
            content_hash: "h".into(),
            total_bytes: 5,
            needs_file_download: false,
            file_download_state: "idle".into(),
            download_token: Some("tok".into()),
            source_host: None,
            source_port: None,
            preview_json: "{}".into(),
        }
    }

    fn sample_file_item(id: &str, pasteboard_id: &str, hash: &str) -> StoredItem {
        StoredItem {
            id: id.into(),
            pasteboard_id: pasteboard_id.into(),
            sort_order: 0,
            item_type: "file".into(),
            text_content: None,
            html_content: None,
            rtf_b64: None,
            url: None,
            color: None,
            blob_hash: Some(hash.into()),
            file_name: Some("a.bin".into()),
            file_size: Some(5),
            width: None,
            height: None,
            download_token: Some("tok".into()),
        }
    }

    #[test]
    fn find_file_by_item_id_or_pasteboard_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let src = dir.path().join("a.bin");
        std::fs::write(&src, b"hello").unwrap();
        let (hash, _) = store.ingest_file(&src).unwrap();
        store
            .insert_pasteboard(
                &sample_pb("pb-1"),
                &[sample_file_item("item-1", "pb-1", &hash)],
            )
            .unwrap();

        let by_item = store.find_file_for_download("item-1").unwrap().unwrap();
        let by_pb = store.find_file_for_download("pb-1").unwrap().unwrap();
        assert_eq!(by_item.id, "item-1");
        assert_eq!(by_item.blob_hash.as_deref(), Some(hash.as_str()));
        assert_eq!(by_pb.id, "item-1");
        assert!(store.find_file_for_download("missing").unwrap().is_none());
    }

    #[test]
    fn sanitize_keeps_unicode_basename_and_extension() {
        assert_eq!(
            sanitize_file_name(Some("新增 文本文档.txt")),
            "新增 文本文档.txt"
        );
        assert_eq!(sanitize_file_name(Some("a/../b.txt")), "b.txt");
        assert_eq!(sanitize_file_name(Some("")), "file");
        assert_eq!(sanitize_file_name(None), "file");
    }

    #[test]
    fn named_blob_path_uses_original_filename() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let src = dir.path().join("新增 文本文档.txt");
        std::fs::write(&src, b"hello").unwrap();
        let (hash, _) = store.ingest_file(&src).unwrap();
        let named = store
            .named_blob_path(&hash, Some("新增 文本文档.txt"))
            .unwrap();
        assert_eq!(
            named.file_name().unwrap().to_string_lossy(),
            "新增 文本文档.txt"
        );
        assert_eq!(std::fs::read(&named).unwrap(), b"hello");
    }
}
