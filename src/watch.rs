use anyhow::Result;
use notify::{Event, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;

use crate::config::Config;
use crate::db::Database;
use crate::embed::Embedder;
use crate::ingest;
use crate::memory as mem_extract;
use crate::util;

pub async fn watch_paths(
    paths: &[String],
    config: &Config,
    db: &Database,
    embedder: &Embedder,
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = notify::recommended_watcher(tx)?;

    for path_str in paths {
        let path = expand_path(path_str);
        if path.exists() {
            watcher.watch(&path, RecursiveMode::Recursive)?;
            eprintln!("Watching: {}", path.display());
        } else {
            eprintln!("Warning: path does not exist: {path_str}");
        }
    }

    eprintln!("Press Ctrl+C to stop watching.");

    let extensions: Vec<String> = config.index.default_extensions.clone();
    let max_size = crate::config::parse_max_file_size(&config.index.max_file_size);

    for res in rx {
        match res {
            Ok(event) => {
                use notify::EventKind::*;
                match event.kind {
                    Create(_) | Modify(_) => {
                        for path in &event.paths {
                            if should_process(path, &extensions, max_size) {
                                eprintln!("Changed: {}", path.display());
                                if let Err(e) = reindex_file(path, config, db, embedder).await {
                                    eprintln!("Error indexing {}: {e}", path.display());
                                }
                            }
                        }
                    }
                    Remove(_) => {
                        for path in &event.paths {
                            let abs = path.canonicalize().unwrap_or_else(|_| path.clone());
                            let abs_str = abs.to_string_lossy().to_string();
                            if let Ok(Some(file)) = db.get_file_by_path(&abs_str) {
                                eprintln!("Removed: {}", path.display());
                                let _ = db.delete_file_data(&file.file_id);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => eprintln!("Watch error: {e}"),
        }
    }

    Ok(())
}

async fn reindex_file(
    path: &PathBuf,
    config: &Config,
    db: &Database,
    embedder: &Embedder,
) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let abs = path.canonicalize().unwrap_or_else(|_| path.clone());
    let abs_str = abs.to_string_lossy().to_string();

    let content_hash = util::hash_content(&content);

    // Check if file changed
    if let Some(existing) = db.get_file_by_path(&abs_str)? {
        if existing.content_hash == content_hash {
            return Ok(()); // No change
        }
        db.delete_file_data(&existing.file_id)?;
    }

    // Re-index
    let file_record = ingest::make_file_record(path, &content);
    db.upsert_file(&file_record)?;

    let chunks = ingest::chunk_file(path, &content, &file_record.file_id);

    let mut texts_to_embed = Vec::new();
    let mut chunk_ids_to_embed = Vec::new();

    for chunk in &chunks {
        db.insert_chunk(chunk)?;
        if embedder.is_available() {
            texts_to_embed.push(chunk.content.clone());
            chunk_ids_to_embed.push(chunk.chunk_id.clone());
        }
        if config.memory.enabled {
            let memories = mem_extract::extract_memories(chunk);
            for mem in &memories {
                if mem.importance >= config.memory.min_importance {
                    db.insert_memory(mem)?;
                }
            }
        }
    }

    // Embed
    if embedder.is_available() && !texts_to_embed.is_empty() {
        match embedder.embed_batch(&texts_to_embed).await {
            Ok(vecs) => {
                for (cid, vec) in chunk_ids_to_embed.iter().zip(vecs.iter()) {
                    db.store_embedding(cid, vec.len(), vec)?;
                }
            }
            Err(e) => eprintln!("Embedding error: {e}"),
        }
    }

    db.set_state("last_indexed_at", &util::now_iso())?;
    eprintln!("Indexed: {} ({} chunks)", path.display(), chunks.len());

    Ok(())
}

fn expand_path(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(rest)
    } else {
        PathBuf::from(s)
    }
}

fn should_process(path: &Path, extensions: &[String], max_size: u64) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !extensions.iter().any(|e| e.to_lowercase() == ext) {
        return false;
    }
    if let Ok(meta) = path.metadata() {
        if meta.len() > max_size {
            return false;
        }
    }
    true
}
