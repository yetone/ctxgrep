# ctxgrep — Context & Memory Retrieval Skill

## Overview

ctxgrep is a grep-style CLI for searching documents, notes, memories, and project context. Use it to find relevant background material, recall decisions, and assemble context bundles for tasks.

## When to use

- You need to find relevant documents, notes, or knowledge from a local corpus
- You need to recall past decisions, preferences, or constraints
- You need to assemble a context bundle for a task within a token budget
- You need to search across a knowledge base using exact, regex, or semantic queries

## Prerequisites

- `ctxgrep` must be installed and available in PATH
- For semantic search: `OPENAI_API_KEY` environment variable must be set
- Documents must be indexed first with `ctxgrep index`

## Commands

### Index documents

```bash
# Index specific directories
ctxgrep index ~/notes ~/docs --recursive

# Rebuild from scratch
ctxgrep index ~/notes --recursive --rebuild

# Index with memory extraction
ctxgrep index ~/notes --recursive --with-memory
```

### Search

```bash
# Hybrid search (default, best quality)
ctxgrep search --json "your query"

# Exact text match
ctxgrep search --exact --json "exact phrase"

# Regex match
ctxgrep search --regex --json "pattern.*here"

# Semantic search
ctxgrep search --semantic --json "conceptual query"

# Control result count
ctxgrep search --json --top-k 5 "query"
```

### Memory search

```bash
# Search all memories
ctxgrep memory --json "topic"

# Filter by type
ctxgrep memory --json --type decision "topic"
ctxgrep memory --json --type preference "topic"
ctxgrep memory --json --type constraint "topic"
```

### Context packing

```bash
# Pack context within token budget
ctxgrep pack --json --budget 4000 "task description"

# Include source snippets
ctxgrep pack --json --budget 4000 --include-snippets "task description"
```

### Status and diagnostics

```bash
ctxgrep status
ctxgrep doctor
```

## Output format

All commands support `--json` for structured output. The JSON output includes:

- `chunk_id` — unique identifier for the text chunk
- `path` — source file path
- `title` — section title
- `section_path` — hierarchical section path (e.g., "Chapter > Section > Subsection")
- `start_line` / `end_line` — line numbers in source file
- `score` — relevance score (0.0–1.0)
- `snippet` — text preview
- `why` — list of reasons the result matched

## Best practices for agents

1. **Always use `--json`** for machine-readable output
2. **Index before searching** — run `ctxgrep status` to check if documents are indexed
3. **Use hybrid search** (default) for best recall quality
4. **Use `pack` for task preparation** — it deduplicates and budgets automatically
5. **Check `why` field** to understand match quality
6. **Use `score` to filter** — results below 0.3 are likely noise
7. **Memory search for decisions** — use `ctxgrep memory --type decision` to find past decisions

## Example workflow

```bash
# 1. Check if index exists
ctxgrep status

# 2. Index if needed
ctxgrep index ~/project-docs --recursive

# 3. Search for relevant context
ctxgrep search --json "authentication design decisions"

# 4. Get specific memories
ctxgrep memory --json --type decision "authentication"

# 5. Pack context for a task
ctxgrep pack --json --budget 5000 "implement OAuth2 flow based on previous design decisions"
```
