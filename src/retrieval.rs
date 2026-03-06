use anyhow::Result;
use std::collections::HashMap;

use crate::config::Config;
use crate::db::Database;
use crate::embed::{cosine_similarity, Embedder};
use crate::model::*;
use crate::util;

pub async fn search(
    db: &Database,
    embedder: &Embedder,
    query: &str,
    opts: &SearchOptions,
    config: &Config,
) -> Result<Vec<SearchResult>> {
    match opts.mode {
        SearchMode::Exact => search_exact(db, query, opts),
        SearchMode::Regex => search_regex(db, query, opts),
        SearchMode::Semantic => search_semantic(db, embedder, query, opts).await,
        SearchMode::Hybrid => search_hybrid(db, embedder, query, opts, config).await,
    }
}

// ── Exact ──

fn search_exact(db: &Database, query: &str, opts: &SearchOptions) -> Result<Vec<SearchResult>> {
    let chunk_ids = db.search_exact(query, opts.top_k * 3)?;
    let mut results = Vec::new();
    for cid in chunk_ids {
        if let Some(chunk) = db.get_chunk(&cid)? {
            let path = db.get_file_path_for_chunk(&cid)?.unwrap_or_default();
            let updated = db.get_file_updated_at_for_chunk(&cid)?;
            let mut why = vec!["exact match".to_string()];
            if let Some(t) = &chunk.title {
                if t.to_lowercase().contains(&query.to_lowercase()) {
                    why.push("title match".to_string());
                }
            }
            results.push(SearchResult {
                doc_id: chunk.doc_id.clone(),
                chunk_id: chunk.chunk_id.clone(),
                path,
                title: chunk.title.clone(),
                section_path: chunk.section_path.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                score: 1.0,
                lexical_score: 1.0,
                semantic_score: 0.0,
                recency_score: 0.0,
                importance_score: 0.0,
                snippet: chunk
                    .content_preview
                    .clone()
                    .unwrap_or_else(|| util::truncate(&chunk.content, 200)),
                why,
                updated_at: updated,
                result_type: ResultType::Chunk,
            });
        }
    }
    results.truncate(opts.top_k);
    Ok(results)
}

// ── Regex ──

fn search_regex(db: &Database, query: &str, opts: &SearchOptions) -> Result<Vec<SearchResult>> {
    let re = regex::Regex::new(query)?;
    let all = db.all_chunk_ids_and_content()?;
    let mut results = Vec::new();

    for (cid, content) in &all {
        if re.is_match(content) {
            if let Some(chunk) = db.get_chunk(cid)? {
                let path = db.get_file_path_for_chunk(cid)?.unwrap_or_default();
                let updated = db.get_file_updated_at_for_chunk(cid)?;
                results.push(SearchResult {
                    doc_id: chunk.doc_id.clone(),
                    chunk_id: chunk.chunk_id.clone(),
                    path,
                    title: chunk.title.clone(),
                    section_path: chunk.section_path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    score: 1.0,
                    lexical_score: 1.0,
                    semantic_score: 0.0,
                    recency_score: 0.0,
                    importance_score: 0.0,
                    snippet: chunk
                        .content_preview
                        .clone()
                        .unwrap_or_else(|| util::truncate(&chunk.content, 200)),
                    why: vec!["regex match".to_string()],
                    updated_at: updated,
                    result_type: ResultType::Chunk,
                });
            }
        }
        if results.len() >= opts.top_k {
            break;
        }
    }

    Ok(results)
}

// ── Semantic ──

async fn search_semantic(
    db: &Database,
    embedder: &Embedder,
    query: &str,
    opts: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    if !embedder.is_available() {
        anyhow::bail!("Semantic search requires an embedding provider. Set OPENAI_API_KEY or configure embedding in ~/.ctxgrep/config.toml");
    }

    let query_vec = embedder.embed_single(query).await?;
    let all_embeddings = db.get_all_embeddings()?;

    let mut scored: Vec<(String, f32)> = all_embeddings
        .iter()
        .map(|(cid, vec)| (cid.clone(), cosine_similarity(&query_vec, vec)))
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(opts.top_k);

    let mut results = Vec::new();
    for (cid, sim) in scored {
        if let Some(chunk) = db.get_chunk(&cid)? {
            let path = db.get_file_path_for_chunk(&cid)?.unwrap_or_default();
            let updated = db.get_file_updated_at_for_chunk(&cid)?;
            results.push(SearchResult {
                doc_id: chunk.doc_id.clone(),
                chunk_id: chunk.chunk_id.clone(),
                path,
                title: chunk.title.clone(),
                section_path: chunk.section_path.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                score: sim as f64,
                lexical_score: 0.0,
                semantic_score: sim as f64,
                recency_score: 0.0,
                importance_score: 0.0,
                snippet: chunk
                    .content_preview
                    .clone()
                    .unwrap_or_else(|| util::truncate(&chunk.content, 200)),
                why: vec![format!("semantic similarity: {sim:.3}")],
                updated_at: updated,
                result_type: ResultType::Chunk,
            });
        }
    }

    Ok(results)
}

// ── Hybrid ──

async fn search_hybrid(
    db: &Database,
    embedder: &Embedder,
    query: &str,
    opts: &SearchOptions,
    config: &Config,
) -> Result<Vec<SearchResult>> {
    let w = &config.retrieval;

    // Lexical retrieval via FTS
    let fts_results = db.search_chunks_fts(query, opts.top_k * 3)?;
    let mut score_map: HashMap<String, ScoreAccum> = HashMap::new();

    for (cid, lex_score) in &fts_results {
        score_map.entry(cid.clone()).or_default().lexical = *lex_score;
        score_map
            .get_mut(cid)
            .unwrap()
            .why
            .push("lexical match".to_string());
    }

    // Semantic retrieval
    if embedder.is_available() {
        if let Ok(query_vec) = embedder.embed_single(query).await {
            let all_embeddings = db.get_all_embeddings()?;
            let mut semantic_scores: Vec<(String, f32)> = all_embeddings
                .iter()
                .map(|(cid, vec)| (cid.clone(), cosine_similarity(&query_vec, vec)))
                .collect();
            semantic_scores
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            semantic_scores.truncate(opts.top_k * 3);

            for (cid, sim) in semantic_scores {
                let entry = score_map.entry(cid).or_default();
                entry.semantic = sim as f64;
                entry.why.push(format!("semantic similarity: {sim:.3}"));
            }
        }
    }

    // Memory retrieval (boost chunks that have associated memories)
    let mem_results = db.search_memories_fts(query, opts.top_k)?;
    for (mid, _score) in &mem_results {
        if let Some(mem) = db.get_memory(mid)? {
            let entry = score_map.entry(mem.source_chunk_id.clone()).or_default();
            entry.importance = entry.importance.max(mem.importance);
            entry
                .why
                .push(format!("memory hit: {}", mem.memory_type.as_str()));
        }
    }

    // Compute recency and scope scores, then fuse
    let mut results = Vec::new();
    for (cid, accum) in &score_map {
        if let Some(chunk) = db.get_chunk(cid)? {
            let path = db.get_file_path_for_chunk(cid)?.unwrap_or_default();
            let updated = db.get_file_updated_at_for_chunk(cid)?;

            let recency = compute_recency(updated.as_deref());
            let scope = compute_scope(
                query,
                chunk.title.as_deref(),
                chunk.section_path.as_deref(),
                &path,
            );

            let final_score = w.lexical_weight * accum.lexical
                + w.semantic_weight * accum.semantic
                + w.recency_weight * recency
                + w.importance_weight * accum.importance
                + w.scope_weight * scope;

            results.push(SearchResult {
                doc_id: chunk.doc_id.clone(),
                chunk_id: chunk.chunk_id.clone(),
                path,
                title: chunk.title.clone(),
                section_path: chunk.section_path.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                score: final_score,
                lexical_score: accum.lexical,
                semantic_score: accum.semantic,
                recency_score: recency,
                importance_score: accum.importance,
                snippet: chunk
                    .content_preview
                    .clone()
                    .unwrap_or_else(|| util::truncate(&chunk.content, 200)),
                why: accum.why.clone(),
                updated_at: updated,
                result_type: ResultType::Chunk,
            });
        }
    }

    // Rerank
    rerank(&mut results, query);
    results.truncate(opts.top_k);
    Ok(results)
}

// ── Score accumulator ──

#[derive(Default)]
struct ScoreAccum {
    lexical: f64,
    semantic: f64,
    importance: f64,
    why: Vec<String>,
}

// ── Recency ──

fn compute_recency(updated_at: Option<&str>) -> f64 {
    let Some(ts) = updated_at else {
        return 0.3;
    };
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) else {
        return 0.3;
    };
    let age_days = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc))
        .num_days()
        .max(0) as f64;
    // Decay: 1.0 for today, ~0.5 for 30 days ago, ~0.2 for 180 days
    (1.0 / (1.0 + age_days / 30.0)).min(1.0)
}

// ── Scope score ──

fn compute_scope(query: &str, title: Option<&str>, section_path: Option<&str>, path: &str) -> f64 {
    let q = query.to_lowercase();
    let mut score: f64 = 0.0;
    if let Some(t) = title {
        if t.to_lowercase().contains(&q) {
            score += 0.5;
        }
    }
    if let Some(sp) = section_path {
        if sp.to_lowercase().contains(&q) {
            score += 0.3;
        }
    }
    if path.to_lowercase().contains(&q) {
        score += 0.2;
    }
    score.min(1.0)
}

// ── Rerank ──

fn rerank(results: &mut [SearchResult], query: &str) {
    let q_lower = query.to_lowercase();
    for r in results.iter_mut() {
        // Title exact match bonus
        if let Some(t) = &r.title {
            if t.to_lowercase().contains(&q_lower) {
                r.score += 0.05;
                if !r.why.iter().any(|w| w.contains("title")) {
                    r.why.push("title match".to_string());
                }
            }
        }
        // Exact phrase in content bonus
        if r.snippet.to_lowercase().contains(&q_lower) {
            r.score += 0.03;
            if !r.why.iter().any(|w| w.contains("exact phrase")) {
                r.why.push("contains exact phrase".to_string());
            }
        }
    }
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}
