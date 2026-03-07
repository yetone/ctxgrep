---
name: ctxgrep
description: >
  Context and memory retrieval CLI for searching documents, notes, and project
  knowledge. Use when needing to find relevant documents from a local corpus,
  recall past decisions/preferences/constraints, assemble context bundles within
  token budgets, or search across a knowledge base using exact, regex, semantic,
  or hybrid queries. Triggers on: context retrieval, knowledge search, memory
  lookup, decision recall, document search, context packing, "what did we decide",
  "find relevant notes", "search project docs".
---

# ctxgrep

grep-style CLI for context and memory retrieval. Combines exact search, regex, semantic retrieval, and heuristic memory extraction.

## Prerequisites

- `ctxgrep` installed and in PATH
- Documents indexed via `ctxgrep index`
- No API key needed — uses local embedding model by default (auto-downloads on first run)

## Quick Reference

```bash
# Index
ctxgrep index <paths...> --recursive [--rebuild] [--with-memory]

# Search (always use --json for agent consumption)
ctxgrep search --json "query"                    # hybrid (default, best quality)
ctxgrep search --json --exact "exact phrase"     # exact match
ctxgrep search --json --regex "pattern.*"        # regex
ctxgrep search --json --semantic "concept"       # vector similarity
ctxgrep search --json --top-k 5 "query"          # limit results

# Memory (extracted decisions, constraints, preferences, TODOs, etc.)
ctxgrep memory --json "topic"
ctxgrep memory --json --type decision "topic"    # filter by type
ctxgrep memory --json --type constraint "topic"
ctxgrep memory --json --type preference "topic"
ctxgrep memory --json --type todo "topic"
ctxgrep memory --json --type definition "topic"
ctxgrep memory --json --type fact "topic"

# Context packing (deduplicates + budgets automatically)
ctxgrep pack --json --budget 4000 "task description"

# Diagnostics
ctxgrep status
ctxgrep doctor
```

## JSON Output Fields

- `path` — source file path
- `title` — section title
- `score` — relevance score (0.0–1.0); below 0.3 is likely noise
- `snippet` — text content
- `why` — match explanation (e.g., "lexical match", "semantic similarity: 0.65")

## Agent Workflow

```bash
# 1. Check index state
ctxgrep status

# 2. Index if needed (only once per corpus)
ctxgrep index ~/project-docs --recursive

# 3. Search for context
ctxgrep search --json "authentication design"

# 4. Recall specific decisions
ctxgrep memory --json --type decision "authentication"

# 5. Pack context for a task with token budget
ctxgrep pack --json --budget 5000 "implement OAuth2 based on past decisions"
```

## Guidelines

1. Always use `--json` for structured, parseable output.
2. Use hybrid search (default) for best recall. Use `--exact` only when you know the precise phrase.
3. Use `pack` for task preparation — it deduplicates, categorizes memories, and respects token budgets.
4. Filter results by `score` — discard anything below 0.3.
5. Use `memory --type decision` to find past architectural/design decisions before proposing changes.
