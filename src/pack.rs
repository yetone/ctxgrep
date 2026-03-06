use std::collections::HashSet;

use crate::db::Database;
use crate::model::*;
use crate::util;

pub fn pack_results(
    query: &str,
    results: &[SearchResult],
    db: &Database,
    budget: usize,
    include_snippets: bool,
) -> PackResult {
    // Deduplicate by chunk_id
    let mut seen = HashSet::new();
    let deduped: Vec<&SearchResult> = results
        .iter()
        .filter(|r| seen.insert(r.chunk_id.clone()))
        .collect();

    // Collect memories from related chunks
    let mut facts = Vec::new();
    let mut decisions = Vec::new();
    let mut preferences = Vec::new();
    let mut constraints = Vec::new();
    let mut open_questions = Vec::new();
    let mut sources = Vec::new();
    let mut used_tokens = 0usize;

    // Generate summary from top result
    let summary = if let Some(top) = deduped.first() {
        util::truncate(&top.snippet, 300)
    } else {
        "No relevant context found.".to_string()
    };
    used_tokens += util::estimate_tokens(&summary);

    // Gather memories associated with result chunks
    for r in &deduped {
        if used_tokens >= budget {
            break;
        }

        // Try to get memories for this chunk
        if let Ok(chunks) = db.get_chunks_by_file("") {
            // We won't iterate all files; instead check memories via the chunk
            let _ = chunks; // placeholder
        }

        // Collect from the chunk's content for question detection
        if r.snippet.contains('?') || r.snippet.contains('？') {
            let q = extract_question(&r.snippet);
            if !q.is_empty() {
                let tok = util::estimate_tokens(&q);
                if used_tokens + tok <= budget {
                    open_questions.push(q);
                    used_tokens += tok;
                }
            }
        }

        // Add source
        sources.push(SourceRef {
            path: r.path.clone(),
            chunk_id: r.chunk_id.clone(),
            score: r.score,
        });
    }

    // Gather memories from DB for result chunks
    for r in &deduped {
        if used_tokens >= budget {
            break;
        }
        if let Ok(Some(chunk)) = db.get_chunk(&r.chunk_id) {
            // Search for memories linked to this chunk
            let memories = get_memories_for_chunk(db, &chunk.chunk_id);
            for mem in memories {
                let tok = util::estimate_tokens(&mem.content);
                if used_tokens + tok > budget {
                    break;
                }
                match mem.memory_type {
                    MemoryType::Fact | MemoryType::Summary => {
                        facts.push(mem.content.clone());
                    }
                    MemoryType::Decision => {
                        decisions.push(mem.content.clone());
                    }
                    MemoryType::Preference => {
                        preferences.push(mem.content.clone());
                    }
                    MemoryType::Constraint => {
                        constraints.push(mem.content.clone());
                    }
                    MemoryType::Todo | MemoryType::Definition => {
                        facts.push(mem.content.clone());
                    }
                }
                used_tokens += tok;
            }
        }
    }

    // If include_snippets and we have budget left, add top snippets as facts
    if include_snippets {
        for r in &deduped {
            if used_tokens >= budget {
                break;
            }
            let tok = util::estimate_tokens(&r.snippet);
            if used_tokens + tok <= budget && !facts.iter().any(|f| f == &r.snippet) {
                facts.push(r.snippet.clone());
                used_tokens += tok;
            }
        }
    }

    PackResult {
        query: query.to_string(),
        budget_tokens: budget,
        summary,
        facts,
        decisions,
        preferences,
        constraints,
        open_questions,
        sources,
    }
}

fn get_memories_for_chunk(db: &Database, chunk_id: &str) -> Vec<Memory> {
    // Query memories whose source_chunk_id matches
    db.search_memories_by_type("", None, 100)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.source_chunk_id == chunk_id)
        .collect()
}

fn extract_question(text: &str) -> String {
    // Find sentences ending with ? or ？
    for sentence in text.split(['.', '。', '\n']) {
        let trimmed = sentence.trim();
        if trimmed.ends_with('?') || trimmed.ends_with('？') {
            return trimmed.to_string();
        }
    }
    String::new()
}
