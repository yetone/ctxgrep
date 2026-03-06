use colored::*;

use crate::model::*;
use crate::util;

// ── Human-readable output ──

pub fn print_search_results(results: &[SearchResult]) {
    if results.is_empty() {
        eprintln!("No results found.");
        return;
    }

    for (i, r) in results.iter().enumerate() {
        if i > 0 {
            println!();
        }

        // Header line: path:lines  [score=X mode]
        let location = if r.start_line > 0 {
            format!("{}:{}-{}", r.path, r.start_line, r.end_line)
        } else {
            r.path.clone()
        };
        let score_str = format!("[score={:.2}]", r.score);

        println!("{}  {}", location.cyan().bold(), score_str.yellow());

        // Title
        if let Some(t) = &r.title {
            println!("  {}", t.white().bold());
        }

        // Snippet
        let snippet = util::truncate(&r.snippet, 300);
        for line in snippet.lines() {
            println!("  {}", line.dimmed());
        }

        // Why
        if !r.why.is_empty() {
            let why_str = r.why.join(", ");
            println!("  {} {}", "why:".green(), why_str);
        }
    }
}

pub fn print_search_results_json(results: &[SearchResult], query: &str, mode: &str) {
    let output = serde_json::json!({
        "query": query,
        "mode": mode,
        "results": results,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

// ── Memory output ──

pub fn print_memories(memories: &[Memory]) {
    if memories.is_empty() {
        eprintln!("No memories found.");
        return;
    }

    for (i, m) in memories.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let type_str = format!("[{}]", m.memory_type.as_str());
        let imp_str = format!(
            "importance={:.2} confidence={:.2}",
            m.importance, m.confidence
        );

        println!("{}  {}", type_str.magenta().bold(), imp_str.yellow());
        println!("  {} {}", "subject:".green(), m.subject);
        println!("  {}", m.content.dimmed());
    }
}

pub fn print_memories_json(memories: &[Memory], query: &str) {
    let output = serde_json::json!({
        "query": query,
        "memories": memories,
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

// ── Pack output ──

pub fn print_pack(pack: &PackResult) {
    println!("{}", "── Context Pack ──".cyan().bold());
    println!();
    println!("{} {}", "Query:".green().bold(), pack.query);
    println!("{} {}", "Budget:".green().bold(), pack.budget_tokens);
    println!();
    println!("{}", "Summary:".white().bold());
    println!("  {}", pack.summary.dimmed());

    print_section("Facts", &pack.facts);
    print_section("Decisions", &pack.decisions);
    print_section("Preferences", &pack.preferences);
    print_section("Constraints", &pack.constraints);
    print_section("Open Questions", &pack.open_questions);

    if !pack.sources.is_empty() {
        println!();
        println!("{}", "Sources:".white().bold());
        for s in &pack.sources {
            println!("  {} [score={:.2}]", s.path.cyan(), s.score);
        }
    }
}

pub fn print_pack_json(pack: &PackResult) {
    println!("{}", serde_json::to_string_pretty(pack).unwrap());
}

fn print_section(title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    println!();
    println!("{}:", title.white().bold());
    for item in items {
        println!("  - {}", item.dimmed());
    }
}

// ── Status output ──

pub fn print_status(stats: &IndexStats) {
    println!("{}", "── ctxgrep status ──".cyan().bold());
    println!("  {} {}", "Files:".green(), stats.total_files);
    println!("  {} {}", "Chunks:".green(), stats.total_chunks);
    println!("  {} {}", "Memories:".green(), stats.total_memories);
    println!("  {} {}", "Embeddings:".green(), stats.total_embeddings);
    println!("  {} {}", "Model:".green(), stats.embedding_model);
    println!("  {} {}", "Database:".green(), stats.db_path);
    if let Some(t) = &stats.last_indexed_at {
        println!("  {} {}", "Last indexed:".green(), t);
    }
}
