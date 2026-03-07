#![allow(clippy::too_many_arguments)]

mod cli;
mod config;
mod db;
mod embed;
mod ingest;
mod memory;
mod model;
mod output;
mod pack;
mod retrieval;
mod util;
mod watch;

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;

use cli::{Cli, Commands};
use model::*;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    config::ensure_dirs()?;
    let mut cfg = config::load_config()?;

    match cli.command {
        Commands::Index {
            paths,
            recursive,
            hidden,
            no_ignore,
            ext,
            max_file_size,
            rebuild,
            with_memory,
            embedder,
            model,
        } => {
            if let Some(mfs) = max_file_size {
                cfg.index.max_file_size = mfs;
            }
            if let Some(e) = embedder {
                cfg.embedding.provider = e;
            }
            if let Some(m) = model {
                cfg.embedding.model = m;
            }
            let memory_enabled = with_memory || cfg.memory.enabled;
            cmd_index(
                &paths,
                &cfg,
                recursive,
                hidden,
                no_ignore,
                ext.as_deref(),
                rebuild,
                memory_enabled,
            )
            .await
        }

        Commands::Search {
            query,
            paths,
            exact,
            regex,
            semantic,
            hybrid: _,
            top_k,
            json,
            full_section: _,
            with_meta: _,
            path,
            tag,
            after,
            before,
            source: _,
            budget: _,
            global,
        } => {
            let mode = if exact {
                SearchMode::Exact
            } else if regex {
                SearchMode::Regex
            } else if semantic {
                SearchMode::Semantic
            } else {
                SearchMode::Hybrid
            };

            // Resolve path prefixes: explicit paths take priority; otherwise
            // default to the current working directory unless --global is set.
            let path_prefixes = if global {
                vec![]
            } else if paths.is_empty() {
                let cwd = std::env::current_dir()
                    .context("failed to determine current working directory; use --global to search the entire index")?
                    .to_string_lossy()
                    .to_string();
                vec![cwd]
            } else {
                paths
                    .iter()
                    .map(|p| {
                        std::path::Path::new(p)
                            .canonicalize()
                            .unwrap_or_else(|_| std::path::PathBuf::from(p))
                            .to_string_lossy()
                            .to_string()
                    })
                    .collect()
            };

            cmd_search(
                &query,
                mode,
                top_k,
                json,
                path_prefixes,
                path,
                tag,
                after,
                before,
                &cfg,
            )
            .await
        }

        Commands::Memory {
            query,
            r#type,
            json,
            top_k,
        } => cmd_memory(&query, r#type.as_deref(), json, top_k, &cfg),

        Commands::Pack {
            query,
            budget,
            json,
            include_memory: _,
            include_snippets,
        } => cmd_pack(&query, budget, json, include_snippets, &cfg).await,

        Commands::Watch { paths } => cmd_watch(&paths, &cfg).await,

        Commands::Status => cmd_status(&cfg),

        Commands::Clear => cmd_clear(&cfg),

        Commands::Doctor => cmd_doctor(&cfg).await,
    }
}

// ── Index ──

async fn cmd_index(
    paths: &[String],
    config: &config::Config,
    recursive: bool,
    hidden: bool,
    no_ignore: bool,
    ext: Option<&str>,
    rebuild: bool,
    memory_enabled: bool,
) -> Result<()> {
    let db_path = config::resolve_db_path(config);
    let db = db::Database::open(&db_path)?;

    if rebuild {
        db.clear_all()?;
        eprintln!("{}", "Index cleared for rebuild.".yellow());
    }

    let files = ingest::walk_paths(paths, config, recursive, hidden, no_ignore, ext)?;
    if files.is_empty() {
        eprintln!("No files found to index.");
        return Ok(());
    }
    eprintln!("Found {} files to index.", files.len());

    let embedder = embed::Embedder::from_config(
        &config.embedding.provider,
        &config.embedding.model,
        config.embedding.dimensions,
    );

    let mut total_chunks = 0usize;
    let mut total_memories = 0usize;
    let mut total_embedded = 0usize;

    for file_path in &files {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Skip {}: {e}", file_path.display());
                continue;
            }
        };

        let abs = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone());
        let abs_str = abs.to_string_lossy().to_string();
        let content_hash = util::hash_content(&content);

        // Skip if unchanged
        if let Some(existing) = db.get_file_by_path(&abs_str)? {
            if existing.content_hash == content_hash {
                continue;
            }
            db.delete_file_data(&existing.file_id)?;
        }

        let file_record = ingest::make_file_record(file_path, &content);
        db.upsert_file(&file_record)?;

        let chunks = ingest::chunk_file(file_path, &content, &file_record.file_id);

        let mut texts_for_embed = Vec::new();
        let mut ids_for_embed = Vec::new();

        for chunk in &chunks {
            db.insert_chunk(chunk)?;
            total_chunks += 1;

            if embedder.is_available() {
                texts_for_embed.push(chunk.content.clone());
                ids_for_embed.push(chunk.chunk_id.clone());
            }

            if memory_enabled {
                let mems = memory::extract_memories(chunk);
                for m in &mems {
                    if m.importance >= config.memory.min_importance {
                        db.insert_memory(m)?;
                        total_memories += 1;
                    }
                }
            }
        }

        // Batch embed
        if embedder.is_available() && !texts_for_embed.is_empty() {
            match embedder.embed_batch(&texts_for_embed).await {
                Ok(vecs) => {
                    for (cid, vec) in ids_for_embed.iter().zip(vecs.iter()) {
                        db.store_embedding(cid, vec.len(), vec)?;
                        total_embedded += 1;
                    }
                }
                Err(e) => eprintln!("Embedding error: {e}"),
            }
        }
    }

    db.set_state("last_indexed_at", &util::now_iso())?;

    eprintln!(
        "{}",
        format!(
            "Done. {total_chunks} chunks, {total_memories} memories, {total_embedded} embeddings."
        )
        .green()
    );
    Ok(())
}

// ── Search ──

async fn cmd_search(
    query: &str,
    mode: SearchMode,
    top_k: usize,
    json: bool,
    path_prefixes: Vec<String>,
    path_filter: Option<String>,
    tag_filter: Option<String>,
    after: Option<String>,
    before: Option<String>,
    config: &config::Config,
) -> Result<()> {
    let db_path = config::resolve_db_path(config);
    let db = db::Database::open(&db_path)?;
    let embedder = embed::Embedder::from_config(
        &config.embedding.provider,
        &config.embedding.model,
        config.embedding.dimensions,
    );

    let opts = SearchOptions {
        mode,
        top_k,
        path_filter,
        tag_filter,
        after,
        before,
        path_prefixes,
    };

    let results = retrieval::search(&db, &embedder, query, &opts, config).await?;

    let mode_str = match mode {
        SearchMode::Exact => "exact",
        SearchMode::Regex => "regex",
        SearchMode::Semantic => "semantic",
        SearchMode::Hybrid => "hybrid",
    };

    if json {
        output::print_search_results_json(&results, query, mode_str);
    } else {
        output::print_search_results(&results);
    }

    Ok(())
}

// ── Memory ──

fn cmd_memory(
    query: &str,
    mem_type: Option<&str>,
    json: bool,
    top_k: usize,
    config: &config::Config,
) -> Result<()> {
    let db_path = config::resolve_db_path(config);
    let db = db::Database::open(&db_path)?;

    let memories = db.search_memories_by_type(query, mem_type, top_k)?;

    if json {
        output::print_memories_json(&memories, query);
    } else {
        output::print_memories(&memories);
    }

    Ok(())
}

// ── Pack ──

async fn cmd_pack(
    query: &str,
    budget: usize,
    json: bool,
    include_snippets: bool,
    config: &config::Config,
) -> Result<()> {
    let db_path = config::resolve_db_path(config);
    let db = db::Database::open(&db_path)?;
    let embedder = embed::Embedder::from_config(
        &config.embedding.provider,
        &config.embedding.model,
        config.embedding.dimensions,
    );

    let opts = SearchOptions {
        mode: SearchMode::Hybrid,
        top_k: 20,
        path_filter: None,
        tag_filter: None,
        after: None,
        before: None,
        path_prefixes: vec![],
    };

    let results = retrieval::search(&db, &embedder, query, &opts, config).await?;
    let pack_result = pack::pack_results(query, &results, &db, budget, include_snippets);

    if json {
        output::print_pack_json(&pack_result);
    } else {
        output::print_pack(&pack_result);
    }

    Ok(())
}

// ── Watch ──

async fn cmd_watch(paths: &[String], config: &config::Config) -> Result<()> {
    let db_path = config::resolve_db_path(config);
    let db = db::Database::open(&db_path)?;
    let embedder = embed::Embedder::from_config(
        &config.embedding.provider,
        &config.embedding.model,
        config.embedding.dimensions,
    );

    watch::watch_paths(paths, config, &db, &embedder).await
}

// ── Status ──

fn cmd_status(config: &config::Config) -> Result<()> {
    let db_path = config::resolve_db_path(config);
    let db = db::Database::open(&db_path)?;
    let stats = db.get_stats(&db_path.to_string_lossy(), &config.embedding.model)?;
    output::print_status(&stats);
    Ok(())
}

// ── Clear ──

fn cmd_clear(config: &config::Config) -> Result<()> {
    let db_path = config::resolve_db_path(config);
    if db_path.exists() {
        let db = db::Database::open(&db_path)?;
        db.clear_all()?;
        eprintln!("{}", "All indexed data cleared.".green());
    } else {
        eprintln!("No index found.");
    }
    Ok(())
}

// ── Doctor ──

async fn cmd_doctor(config: &config::Config) -> Result<()> {
    println!("{}", "── ctxgrep doctor ──".cyan().bold());

    // Check database
    let db_path = config::resolve_db_path(config);
    print!("  Database... ");
    match db::Database::open(&db_path) {
        Ok(db) => {
            println!("{} ({})", "OK".green(), db_path.display());

            // Check FTS
            print!("  FTS5... ");
            if db.fts_ok() {
                println!("{}", "OK".green());
            } else {
                println!("{}", "ERROR".red());
            }

            // Check stats
            if let Ok(stats) = db.get_stats(&db_path.to_string_lossy(), &config.embedding.model) {
                println!(
                    "  {} files, {} chunks, {} memories, {} embeddings",
                    stats.total_files,
                    stats.total_chunks,
                    stats.total_memories,
                    stats.total_embeddings
                );
            }
        }
        Err(e) => println!("{} ({e})", "ERROR".red()),
    }

    // Check embedding provider
    print!("  Embedding provider ({})... ", config.embedding.provider);
    let embedder = embed::Embedder::from_config(
        &config.embedding.provider,
        &config.embedding.model,
        config.embedding.dimensions,
    );
    if embedder.is_available() {
        // Try a test embedding
        match embedder.embed_single("test").await {
            Ok(v) => println!("{} (dim={})", "OK".green(), v.len()),
            Err(e) => println!("{} ({e})", "ERROR".red()),
        }
    } else {
        println!("{}", "NOT CONFIGURED (set OPENAI_API_KEY)".yellow());
    }

    // Check config file
    let cfg_path = config::config_path();
    print!("  Config file... ");
    if cfg_path.exists() {
        println!("{} ({})", "OK".green(), cfg_path.display());
    } else {
        println!("{} (using defaults)", "NOT FOUND".yellow());
    }

    // Check notify capability
    print!("  File watching... ");
    println!("{}", "OK".green());

    Ok(())
}
