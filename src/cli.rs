use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ctxgrep",
    version,
    about = "grep for context, not just text",
    long_about = "ctxgrep is a local-first CLI for searching documents, notes, memories, and project context.\nIt combines exact search, regex, semantic retrieval, and memory extraction to help humans and AI agents pull the right context into the current task."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build or update index
    Index {
        /// Paths to index
        paths: Vec<String>,

        /// Recursive scan
        #[arg(short, long)]
        recursive: bool,

        /// Include hidden files
        #[arg(long)]
        hidden: bool,

        /// Don't respect .gitignore
        #[arg(long)]
        no_ignore: bool,

        /// File extensions to include (comma-separated)
        #[arg(long)]
        ext: Option<String>,

        /// Max file size (e.g. 5MB)
        #[arg(long)]
        max_file_size: Option<String>,

        /// Rebuild index from scratch
        #[arg(long)]
        rebuild: bool,

        /// Also extract memories
        #[arg(long)]
        with_memory: bool,

        /// Embedding provider (openai|none)
        #[arg(long)]
        embedder: Option<String>,

        /// Embedding model name
        #[arg(long)]
        model: Option<String>,
    },

    /// Search indexed documents
    Search {
        /// Search query
        query: String,

        /// Paths or directories to search within (defaults to current directory).
        /// Pass multiple paths to search across several directories at once.
        /// Use --global to search the entire index regardless of location.
        #[arg(value_name = "PATH")]
        paths: Vec<String>,

        /// Exact match mode
        #[arg(long)]
        exact: bool,

        /// Regex match mode
        #[arg(long)]
        regex: bool,

        /// Semantic search mode
        #[arg(long)]
        semantic: bool,

        /// Hybrid search mode (default)
        #[arg(long)]
        hybrid: bool,

        /// Number of results to return
        #[arg(long, default_value = "10")]
        top_k: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Show full section content
        #[arg(long)]
        full_section: bool,

        /// Include metadata in output
        #[arg(long)]
        with_meta: bool,

        /// Filter by path glob (e.g. "*.md")
        #[arg(long)]
        path: Option<String>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,

        /// Filter: after date
        #[arg(long)]
        after: Option<String>,

        /// Filter: before date
        #[arg(long)]
        before: Option<String>,

        /// Filter by source type
        #[arg(long)]
        source: Option<String>,

        /// Token budget limit
        #[arg(long)]
        budget: Option<usize>,

        /// Search the entire index across all directories (disables the default
        /// current-directory scope)
        #[arg(long)]
        global: bool,
    },

    /// Search extracted memories
    Memory {
        /// Search query
        query: String,

        /// Filter by memory type (fact|decision|preference|definition|constraint|todo|summary)
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Number of results
        #[arg(long, default_value = "10")]
        top_k: usize,
    },

    /// Pack context for a task
    Pack {
        /// Task description / query
        query: String,

        /// Token budget
        #[arg(long, default_value = "4000")]
        budget: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Include memory objects
        #[arg(long)]
        include_memory: bool,

        /// Include text snippets
        #[arg(long)]
        include_snippets: bool,
    },

    /// Watch paths for changes and update index
    Watch {
        /// Paths to watch
        paths: Vec<String>,
    },

    /// Show index status
    Status,

    /// Clear all indexed data
    Clear,

    /// Check environment and configuration
    Doctor,
}
