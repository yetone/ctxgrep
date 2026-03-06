use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use crate::model::*;
use crate::util;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    file_id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    source_type TEXT NOT NULL DEFAULT 'file',
    mime_type TEXT,
    language TEXT,
    content_hash TEXT NOT NULL,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    created_at TEXT,
    updated_at TEXT,
    indexed_at TEXT NOT NULL,
    tags_json TEXT DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS chunks (
    chunk_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    doc_id TEXT,
    title TEXT,
    section_path TEXT,
    chunk_type TEXT NOT NULL,
    ordinal INTEGER NOT NULL DEFAULT 0,
    content TEXT NOT NULL,
    content_preview TEXT,
    token_count INTEGER NOT NULL DEFAULT 0,
    start_line INTEGER NOT NULL DEFAULT 0,
    end_line INTEGER NOT NULL DEFAULT 0,
    heading_level INTEGER,
    speaker TEXT,
    timestamp TEXT,
    tags_json TEXT DEFAULT '[]',
    FOREIGN KEY(file_id) REFERENCES files(file_id)
);

CREATE TABLE IF NOT EXISTS chunk_embeddings (
    chunk_id TEXT PRIMARY KEY,
    dim INTEGER NOT NULL,
    vector BLOB NOT NULL,
    FOREIGN KEY(chunk_id) REFERENCES chunks(chunk_id)
);

CREATE TABLE IF NOT EXISTS memories (
    memory_id TEXT PRIMARY KEY,
    source_chunk_id TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    subject TEXT NOT NULL,
    normalized_subject TEXT,
    content TEXT NOT NULL,
    importance REAL NOT NULL DEFAULT 0.5,
    confidence REAL NOT NULL DEFAULT 0.5,
    valid_from TEXT,
    valid_to TEXT,
    extracted_at TEXT NOT NULL,
    tags_json TEXT DEFAULT '[]',
    FOREIGN KEY(source_chunk_id) REFERENCES chunks(chunk_id)
);

CREATE TABLE IF NOT EXISTS index_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id);
CREATE INDEX IF NOT EXISTS idx_memories_chunk ON memories(source_chunk_id);
CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
"#;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Database { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(SCHEMA)
            .context("Failed to initialize schema")?;
        // FTS tables need separate creation since IF NOT EXISTS works differently
        self.ensure_fts()?;
        Ok(())
    }

    fn ensure_fts(&self) -> Result<()> {
        let has_chunks_fts: bool = self
            .conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='chunks_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !has_chunks_fts {
            self.conn.execute_batch(
                "CREATE VIRTUAL TABLE chunks_fts USING fts5(chunk_id UNINDEXED, title, section_path, content, tokenize='unicode61');",
            )?;
        }
        let has_mem_fts: bool = self
            .conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='memories_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !has_mem_fts {
            self.conn.execute_batch(
                "CREATE VIRTUAL TABLE memories_fts USING fts5(memory_id UNINDEXED, subject, content, tokenize='unicode61');",
            )?;
        }
        Ok(())
    }

    // ── Files ──

    pub fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO files (file_id, path, source_type, mime_type, language, content_hash, size_bytes, created_at, updated_at, indexed_at, tags_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                file.file_id,
                file.path,
                file.source_type,
                file.mime_type,
                file.language,
                file.content_hash,
                file.size_bytes,
                file.created_at,
                file.updated_at,
                file.indexed_at,
                serde_json::to_string(&file.tags)?,
            ],
        )?;
        Ok(())
    }

    pub fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_id, path, source_type, mime_type, language, content_hash, size_bytes, created_at, updated_at, indexed_at, tags_json FROM files WHERE path = ?1",
        )?;
        let mut rows = stmt.query(params![path])?;
        match rows.next()? {
            Some(row) => {
                let tags_json: String = row.get(10)?;
                Ok(Some(FileRecord {
                    file_id: row.get(0)?,
                    path: row.get(1)?,
                    source_type: row.get(2)?,
                    mime_type: row.get(3)?,
                    language: row.get(4)?,
                    content_hash: row.get(5)?,
                    size_bytes: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    indexed_at: row.get(9)?,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                }))
            }
            None => Ok(None),
        }
    }

    // ── Chunks ──

    pub fn insert_chunk(&self, chunk: &Chunk) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO chunks (chunk_id, file_id, doc_id, title, section_path, chunk_type, ordinal, content, content_preview, token_count, start_line, end_line, heading_level, speaker, timestamp, tags_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                chunk.chunk_id,
                chunk.file_id,
                chunk.doc_id,
                chunk.title,
                chunk.section_path,
                chunk.chunk_type.as_str(),
                chunk.ordinal,
                chunk.content,
                chunk.content_preview,
                chunk.token_count,
                chunk.start_line,
                chunk.end_line,
                chunk.heading_level,
                chunk.speaker,
                chunk.timestamp,
                serde_json::to_string(&chunk.tags)?,
            ],
        )?;
        // Sync FTS
        self.conn.execute(
            "DELETE FROM chunks_fts WHERE chunk_id = ?1",
            params![chunk.chunk_id],
        )?;
        self.conn.execute(
            "INSERT INTO chunks_fts (chunk_id, title, section_path, content) VALUES (?1, ?2, ?3, ?4)",
            params![
                chunk.chunk_id,
                chunk.title.as_deref().unwrap_or(""),
                chunk.section_path.as_deref().unwrap_or(""),
                chunk.content,
            ],
        )?;
        Ok(())
    }

    pub fn get_chunk(&self, chunk_id: &str) -> Result<Option<Chunk>> {
        let mut stmt = self.conn.prepare(
            "SELECT chunk_id, file_id, doc_id, title, section_path, chunk_type, ordinal, content, content_preview, token_count, start_line, end_line, heading_level, speaker, timestamp, tags_json FROM chunks WHERE chunk_id = ?1",
        )?;
        let mut rows = stmt.query(params![chunk_id])?;
        match rows.next()? {
            Some(row) => {
                let tags_json: String = row.get(15)?;
                Ok(Some(Chunk {
                    chunk_id: row.get(0)?,
                    file_id: row.get(1)?,
                    doc_id: row.get(2)?,
                    title: row.get(3)?,
                    section_path: row.get(4)?,
                    chunk_type: ChunkType::parse(&row.get::<_, String>(5)?),
                    ordinal: row.get(6)?,
                    content: row.get(7)?,
                    content_preview: row.get(8)?,
                    token_count: row.get(9)?,
                    start_line: row.get(10)?,
                    end_line: row.get(11)?,
                    heading_level: row.get(12)?,
                    speaker: row.get(13)?,
                    timestamp: row.get(14)?,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                }))
            }
            None => Ok(None),
        }
    }

    pub fn get_chunks_by_file(&self, file_id: &str) -> Result<Vec<Chunk>> {
        let mut stmt = self.conn.prepare(
            "SELECT chunk_id, file_id, doc_id, title, section_path, chunk_type, ordinal, content, content_preview, token_count, start_line, end_line, heading_level, speaker, timestamp, tags_json FROM chunks WHERE file_id = ?1 ORDER BY ordinal",
        )?;
        let mut rows = stmt.query(params![file_id])?;
        let mut chunks = Vec::new();
        while let Some(row) = rows.next()? {
            let tags_json: String = row.get(15)?;
            chunks.push(Chunk {
                chunk_id: row.get(0)?,
                file_id: row.get(1)?,
                doc_id: row.get(2)?,
                title: row.get(3)?,
                section_path: row.get(4)?,
                chunk_type: ChunkType::parse(&row.get::<_, String>(5)?),
                ordinal: row.get(6)?,
                content: row.get(7)?,
                content_preview: row.get(8)?,
                token_count: row.get(9)?,
                start_line: row.get(10)?,
                end_line: row.get(11)?,
                heading_level: row.get(12)?,
                speaker: row.get(13)?,
                timestamp: row.get(14)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            });
        }
        Ok(chunks)
    }

    // ── Memories ──

    pub fn insert_memory(&self, mem: &Memory) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO memories (memory_id, source_chunk_id, memory_type, subject, normalized_subject, content, importance, confidence, valid_from, valid_to, extracted_at, tags_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                mem.memory_id,
                mem.source_chunk_id,
                mem.memory_type.as_str(),
                mem.subject,
                mem.normalized_subject,
                mem.content,
                mem.importance,
                mem.confidence,
                mem.valid_from,
                mem.valid_to,
                mem.extracted_at,
                serde_json::to_string(&mem.tags)?,
            ],
        )?;
        self.conn.execute(
            "DELETE FROM memories_fts WHERE memory_id = ?1",
            params![mem.memory_id],
        )?;
        self.conn.execute(
            "INSERT INTO memories_fts (memory_id, subject, content) VALUES (?1, ?2, ?3)",
            params![mem.memory_id, mem.subject, mem.content],
        )?;
        Ok(())
    }

    pub fn get_memory(&self, memory_id: &str) -> Result<Option<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT memory_id, source_chunk_id, memory_type, subject, normalized_subject, content, importance, confidence, valid_from, valid_to, extracted_at, tags_json FROM memories WHERE memory_id = ?1",
        )?;
        let mut rows = stmt.query(params![memory_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(self.row_to_memory(row)?)),
            None => Ok(None),
        }
    }

    fn row_to_memory(&self, row: &rusqlite::Row) -> Result<Memory> {
        let tags_json: String = row.get(11)?;
        let mt: String = row.get(2)?;
        Ok(Memory {
            memory_id: row.get(0)?,
            source_chunk_id: row.get(1)?,
            memory_type: MemoryType::parse(&mt).unwrap_or(MemoryType::Fact),
            subject: row.get(3)?,
            normalized_subject: row.get(4)?,
            content: row.get(5)?,
            importance: row.get(6)?,
            confidence: row.get(7)?,
            valid_from: row.get(8)?,
            valid_to: row.get(9)?,
            extracted_at: row.get(10)?,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        })
    }

    // ── Embeddings ──

    pub fn store_embedding(&self, chunk_id: &str, dim: usize, vector: &[f32]) -> Result<()> {
        let blob = bytemuck_cast_slice(vector);
        self.conn.execute(
            "INSERT OR REPLACE INTO chunk_embeddings (chunk_id, dim, vector) VALUES (?1, ?2, ?3)",
            params![chunk_id, dim as i32, blob],
        )?;
        Ok(())
    }

    pub fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT chunk_id, vector FROM chunk_embeddings")?;
        let mut rows = stmt.query([])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let chunk_id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            let vec = bytemuck_cast_to_f32(&blob);
            results.push((chunk_id, vec));
        }
        Ok(results)
    }

    // ── FTS Search ──

    pub fn search_chunks_fts(&self, query: &str, limit: usize) -> Result<Vec<(String, f64)>> {
        // Try FTS5 first
        let fts_query = sanitize_fts_query(query);
        let mut results = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT chunk_id, rank FROM chunks_fts WHERE chunks_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        ) {
            if let Ok(mut rows) = stmt.query(params![fts_query, limit as i64]) {
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let rank: f64 = row.get(1)?;
                    results.push((id, (-rank).min(30.0) / 30.0));
                }
            }
        }
        // Fallback to LIKE for CJK and other cases where FTS fails
        if results.is_empty() {
            let pattern = format!("%{query}%");
            let mut stmt = self.conn.prepare(
                "SELECT chunk_id FROM chunks WHERE content LIKE ?1 OR title LIKE ?1 OR section_path LIKE ?1 LIMIT ?2",
            )?;
            let mut rows = stmt.query(params![pattern, limit as i64])?;
            while let Some(row) = rows.next()? {
                results.push((row.get(0)?, 0.5));
            }
        }
        Ok(results)
    }

    pub fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<(String, f64)>> {
        // Try FTS5 first
        let fts_query = sanitize_fts_query(query);
        let mut results = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT memory_id, rank FROM memories_fts WHERE memories_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        ) {
            if let Ok(mut rows) = stmt.query(params![fts_query, limit as i64]) {
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let rank: f64 = row.get(1)?;
                    results.push((id, (-rank).min(30.0) / 30.0));
                }
            }
        }
        // Fallback to LIKE for CJK
        if results.is_empty() && !query.is_empty() {
            let pattern = format!("%{query}%");
            let mut stmt = self.conn.prepare(
                "SELECT memory_id FROM memories WHERE subject LIKE ?1 OR content LIKE ?1 LIMIT ?2",
            )?;
            let mut rows = stmt.query(params![pattern, limit as i64])?;
            while let Some(row) = rows.next()? {
                results.push((row.get(0)?, 0.5));
            }
        }
        Ok(results)
    }

    pub fn search_memories_by_type(
        &self,
        query: &str,
        mem_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Memory>> {
        let fts_results = self.search_memories_fts(query, limit * 3)?;
        let mut memories = Vec::new();
        for (mid, _score) in &fts_results {
            if let Some(mem) = self.get_memory(mid)? {
                if let Some(t) = mem_type {
                    if mem.memory_type.as_str() == t {
                        memories.push(mem);
                    }
                } else {
                    memories.push(mem);
                }
            }
            if memories.len() >= limit {
                break;
            }
        }
        Ok(memories)
    }

    // ── Delete ──

    pub fn delete_file_data(&self, file_id: &str) -> Result<()> {
        // Get chunk IDs for FTS cleanup
        let chunk_ids: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT chunk_id FROM chunks WHERE file_id = ?1")?;
            let mut rows = stmt.query(params![file_id])?;
            let mut ids = Vec::new();
            while let Some(row) = rows.next()? {
                ids.push(row.get(0)?);
            }
            ids
        };

        for cid in &chunk_ids {
            self.conn
                .execute("DELETE FROM chunks_fts WHERE chunk_id = ?1", params![cid])?;
            self.conn.execute(
                "DELETE FROM memories_fts WHERE memory_id IN (SELECT memory_id FROM memories WHERE source_chunk_id = ?1)",
                params![cid],
            )?;
            self.conn.execute(
                "DELETE FROM memories WHERE source_chunk_id = ?1",
                params![cid],
            )?;
            self.conn.execute(
                "DELETE FROM chunk_embeddings WHERE chunk_id = ?1",
                params![cid],
            )?;
        }
        self.conn
            .execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;
        self.conn
            .execute("DELETE FROM files WHERE file_id = ?1", params![file_id])?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM chunks_fts; DELETE FROM memories_fts; DELETE FROM chunk_embeddings; DELETE FROM memories; DELETE FROM chunks; DELETE FROM files; DELETE FROM index_state;",
        )?;
        Ok(())
    }

    // ── Stats ──

    pub fn get_stats(&self, db_path: &str, embedding_model: &str) -> Result<IndexStats> {
        let total_files: i64 = self
            .conn
            .query_row("SELECT count(*) FROM files", [], |r| r.get(0))?;
        let total_chunks: i64 = self
            .conn
            .query_row("SELECT count(*) FROM chunks", [], |r| r.get(0))?;
        let total_memories: i64 =
            self.conn
                .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))?;
        let total_embeddings: i64 =
            self.conn
                .query_row("SELECT count(*) FROM chunk_embeddings", [], |r| r.get(0))?;
        let last_indexed_at: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM index_state WHERE key = 'last_indexed_at'",
                [],
                |r| r.get(0),
            )
            .ok();
        Ok(IndexStats {
            total_files,
            total_chunks,
            total_memories,
            total_embeddings,
            embedding_model: embedding_model.to_string(),
            db_path: db_path.to_string(),
            last_indexed_at,
        })
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO index_state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn fts_ok(&self) -> bool {
        self.conn
            .query_row(
                "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'test'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .is_ok()
    }

    pub fn get_file_path_for_chunk(&self, chunk_id: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.path FROM files f JOIN chunks c ON c.file_id = f.file_id WHERE c.chunk_id = ?1",
        )?;
        let mut rows = stmt.query(params![chunk_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn get_file_updated_at_for_chunk(&self, chunk_id: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.updated_at FROM files f JOIN chunks c ON c.file_id = f.file_id WHERE c.chunk_id = ?1",
        )?;
        let mut rows = stmt.query(params![chunk_id])?;
        match rows.next()? {
            Some(row) => Ok(row.get(0)?),
            None => Ok(None),
        }
    }

    /// Exact text search on chunks.content
    pub fn search_exact(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare("SELECT chunk_id FROM chunks WHERE content LIKE ?1 LIMIT ?2")?;
        let mut rows = stmt.query(params![pattern, limit as i64])?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next()? {
            ids.push(row.get(0)?);
        }
        Ok(ids)
    }

    /// Get all chunk IDs (for regex search in application layer)
    pub fn all_chunk_ids_and_content(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT chunk_id, content FROM chunks")?;
        let mut rows = stmt.query([])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push((row.get(0)?, row.get(1)?));
        }
        Ok(results)
    }
}

// ── Helpers ──

fn sanitize_fts_query(query: &str) -> String {
    // Escape special FTS5 characters and wrap tokens in quotes for safety
    let cleaned: String = query
        .chars()
        .map(|c| match c {
            '"' | '\'' | '*' | '(' | ')' | ':' | '^' | '{' | '}' => ' ',
            _ => c,
        })
        .collect();
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    if tokens.is_empty() {
        return "\"\"".to_string();
    }
    tokens
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn bytemuck_cast_slice(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn bytemuck_cast_to_f32(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

#[allow(dead_code)]
fn _unused_util() {
    let _ = util::now_iso();
}
