use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::model::*;
use crate::util;

// ── File Walking ──

pub fn walk_paths(
    paths: &[String],
    config: &Config,
    recursive: bool,
    hidden: bool,
    no_ignore: bool,
    ext_override: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let max_size = crate::config::parse_max_file_size(&config.index.max_file_size);
    let extensions: Vec<String> = if let Some(exts) = ext_override {
        exts.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        config.index.default_extensions.clone()
    };

    let mut files = Vec::new();

    for path_str in paths {
        let path = PathBuf::from(shellexpand(path_str));
        if path.is_file() {
            if should_include_file(&path, &extensions, max_size) {
                files.push(path);
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }

        let mut builder = ignore::WalkBuilder::new(&path);
        builder.hidden(!hidden);
        if no_ignore || !config.index.follow_gitignore {
            builder.git_ignore(false);
            builder.git_global(false);
        }
        if !recursive {
            builder.max_depth(Some(1));
        }

        for entry in builder.build().flatten() {
            let p = entry.path().to_path_buf();
            if p.is_file() && should_include_file(&p, &extensions, max_size) {
                files.push(p);
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn should_include_file(path: &Path, extensions: &[String], max_size: u64) -> bool {
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

fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    s.to_string()
}

// ── Chunking ──

pub fn chunk_file(path: &Path, content: &str, file_id: &str) -> Vec<Chunk> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let raw_chunks = match ext.as_str() {
        "md" => chunk_markdown(content, file_id),
        "org" => chunk_markdown(content, file_id), // simplified: treat org like markdown
        "rst" => chunk_plaintext(content, file_id),
        _ => chunk_plaintext(content, file_id),
    };

    // Split oversized chunks
    let mut final_chunks = Vec::new();
    for chunk in raw_chunks {
        if chunk.token_count > 500 {
            final_chunks.extend(split_oversized_chunk(&chunk));
        } else {
            final_chunks.push(chunk);
        }
    }
    final_chunks
}

fn chunk_markdown(content: &str, file_id: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    let mut chunks = Vec::new();
    let mut section_lines: Vec<String> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_level: Option<i32> = None;
    let mut section_path_parts: Vec<(i32, String)> = Vec::new();
    let mut start_line: i32 = 1;
    let mut ordinal: i32 = 0;

    for (i, line) in lines.iter().enumerate() {
        let line_num = (i + 1) as i32;

        if let Some(level) = heading_level(line) {
            // Flush accumulated section
            if !section_lines.is_empty() {
                let text = section_lines.join("\n");
                if !text.trim().is_empty() {
                    let sp = build_section_path(&section_path_parts);
                    chunks.push(make_chunk(
                        file_id,
                        &text,
                        current_title.as_deref(),
                        sp.as_deref(),
                        if current_level.is_some() {
                            ChunkType::HeadingSection
                        } else {
                            ChunkType::ParagraphBlock
                        },
                        ordinal,
                        start_line,
                        line_num - 1,
                        current_level,
                    ));
                    ordinal += 1;
                }
            }

            // Update section path hierarchy
            while section_path_parts.last().is_some_and(|(l, _)| *l >= level) {
                section_path_parts.pop();
            }
            let title = line.trim_start_matches('#').trim().to_string();
            section_path_parts.push((level, title.clone()));

            current_title = Some(title);
            current_level = Some(level);
            section_lines = vec![line.to_string()];
            start_line = line_num;
        } else {
            section_lines.push(line.to_string());
        }
    }

    // Flush last section
    if !section_lines.is_empty() {
        let text = section_lines.join("\n");
        if !text.trim().is_empty() {
            let sp = build_section_path(&section_path_parts);
            chunks.push(make_chunk(
                file_id,
                &text,
                current_title.as_deref(),
                sp.as_deref(),
                if current_level.is_some() {
                    ChunkType::HeadingSection
                } else {
                    ChunkType::ParagraphBlock
                },
                ordinal,
                start_line,
                lines.len() as i32,
                current_level,
            ));
        }
    }

    if chunks.is_empty() && !content.trim().is_empty() {
        chunks.push(make_chunk(
            file_id,
            content,
            None,
            None,
            ChunkType::GenericWindow,
            0,
            1,
            lines.len() as i32,
            None,
        ));
    }

    chunks
}

fn chunk_plaintext(content: &str, file_id: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    // Split by double newlines (paragraph boundaries)
    let mut chunks = Vec::new();
    let mut para_lines: Vec<String> = Vec::new();
    let mut para_start: i32 = 1;
    let mut ordinal: i32 = 0;

    for (i, line) in lines.iter().enumerate() {
        let line_num = (i + 1) as i32;
        if line.trim().is_empty() && !para_lines.is_empty() {
            let text = para_lines.join("\n");
            let tokens = util::estimate_tokens(&text);
            if tokens >= 10 {
                chunks.push(make_chunk(
                    file_id,
                    &text,
                    None,
                    None,
                    ChunkType::ParagraphBlock,
                    ordinal,
                    para_start,
                    line_num - 1,
                    None,
                ));
                ordinal += 1;
            }
            para_lines.clear();
            para_start = line_num + 1;
        } else {
            if para_lines.is_empty() {
                para_start = line_num;
            }
            para_lines.push(line.to_string());
        }
    }

    // Flush
    if !para_lines.is_empty() {
        let text = para_lines.join("\n");
        if !text.trim().is_empty() {
            chunks.push(make_chunk(
                file_id,
                &text,
                None,
                None,
                ChunkType::ParagraphBlock,
                ordinal,
                para_start,
                lines.len() as i32,
                None,
            ));
        }
    }

    // If no chunks were created (single-paragraph file), make one big chunk
    if chunks.is_empty() && !content.trim().is_empty() {
        chunks.push(make_chunk(
            file_id,
            content,
            None,
            None,
            ChunkType::GenericWindow,
            0,
            1,
            lines.len() as i32,
            None,
        ));
    }

    chunks
}

// ── Helpers ──

fn heading_level(line: &str) -> Option<i32> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level > 6 {
        return None;
    }
    // Must have space after #s
    if trimmed.len() > level && trimmed.as_bytes()[level] == b' ' {
        Some(level as i32)
    } else {
        None
    }
}

fn build_section_path(parts: &[(i32, String)]) -> Option<String> {
    if parts.is_empty() {
        return None;
    }
    Some(
        parts
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join(" > "),
    )
}

fn make_chunk(
    file_id: &str,
    content: &str,
    title: Option<&str>,
    section_path: Option<&str>,
    chunk_type: ChunkType,
    ordinal: i32,
    start_line: i32,
    end_line: i32,
    heading_level: Option<i32>,
) -> Chunk {
    let token_count = util::estimate_tokens(content) as i32;
    let preview = util::truncate(content.trim(), 200);
    Chunk {
        chunk_id: util::generate_id(),
        file_id: file_id.to_string(),
        doc_id: None,
        title: title.map(String::from),
        section_path: section_path.map(String::from),
        chunk_type,
        ordinal,
        content: content.to_string(),
        content_preview: Some(preview),
        token_count,
        start_line,
        end_line,
        heading_level,
        speaker: None,
        timestamp: None,
        tags: Vec::new(),
    }
}

fn split_oversized_chunk(chunk: &Chunk) -> Vec<Chunk> {
    let lines: Vec<&str> = chunk.content.lines().collect();
    let target_tokens = 400;
    let mut result = Vec::new();
    let mut buf = Vec::new();
    let mut buf_tokens = 0;
    let mut sub_start = chunk.start_line;
    let mut sub_ordinal = 0;

    for (i, line) in lines.iter().enumerate() {
        let lt = util::estimate_tokens(line);
        buf.push(*line);
        buf_tokens += lt;

        if buf_tokens >= target_tokens {
            let text = buf.join("\n");
            let line_num = chunk.start_line + i as i32;
            result.push(Chunk {
                chunk_id: util::generate_id(),
                file_id: chunk.file_id.clone(),
                doc_id: chunk.doc_id.clone(),
                title: chunk.title.clone(),
                section_path: chunk.section_path.clone(),
                chunk_type: ChunkType::GenericWindow,
                ordinal: chunk.ordinal * 100 + sub_ordinal,
                content: text.clone(),
                content_preview: Some(util::truncate(&text, 200)),
                token_count: buf_tokens as i32,
                start_line: sub_start,
                end_line: line_num,
                heading_level: chunk.heading_level,
                speaker: None,
                timestamp: None,
                tags: chunk.tags.clone(),
            });
            sub_ordinal += 1;
            buf.clear();
            buf_tokens = 0;
            sub_start = line_num + 1;
        }
    }

    if !buf.is_empty() {
        let text = buf.join("\n");
        result.push(Chunk {
            chunk_id: util::generate_id(),
            file_id: chunk.file_id.clone(),
            doc_id: chunk.doc_id.clone(),
            title: chunk.title.clone(),
            section_path: chunk.section_path.clone(),
            chunk_type: ChunkType::GenericWindow,
            ordinal: chunk.ordinal * 100 + sub_ordinal,
            content: text.clone(),
            content_preview: Some(util::truncate(&text, 200)),
            token_count: buf_tokens as i32,
            start_line: sub_start,
            end_line: chunk.end_line,
            heading_level: chunk.heading_level,
            speaker: None,
            timestamp: None,
            tags: chunk.tags.clone(),
        });
    }

    if result.is_empty() {
        result.push(chunk.clone());
    }
    result
}

// ── File Record creation ──

pub fn make_file_record(path: &Path, content: &str) -> FileRecord {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let abs_str = abs.to_string_lossy().to_string();
    let meta = path.metadata().ok();
    let size = meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
    let updated_at = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
        chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    });

    FileRecord {
        file_id: util::generate_id(),
        path: abs_str,
        source_type: "file".to_string(),
        mime_type: util::mime_from_ext(&path.to_string_lossy()),
        language: util::detect_language(&path.to_string_lossy()),
        content_hash: util::hash_content(content),
        size_bytes: size,
        created_at: updated_at.clone(),
        updated_at,
        indexed_at: util::now_iso(),
        tags: Vec::new(),
    }
}
