---
name: ctxgrep
description: >
  Use ctxgrep for local semantic search, hybrid retrieval, and memory extraction
  across documents, notes, and project context. Triggers on: context retrieval,
  knowledge search, memory lookup, decision recall, document search, context
  packing, "what did we decide", "find relevant notes", "search project docs".
---

# ctxgrep

## When to use this skill

Use this skill when you need to search documents or notes, recall past decisions/preferences/constraints, assemble context bundles within token budgets, or search across a local knowledge base using exact, regex, semantic, or hybrid queries.

## What ctxgrep is

`ctxgrep` is a local-first CLI for searching documents, notes, memories, and project context. It is built for humans and AI agents to pull the right context into the current task without any external API.

Core capabilities:

- grep-like CLI with familiar command-line interface
- Local-first indexing with SQLite + FTS5, no server needed
- Hybrid retrieval combining exact / regex / semantic / hybrid search modes
- Memory extraction that automatically extracts decisions, preferences, constraints from text
- Context packing that assembles a token-budgeted context bundle for agents
- JSON output for structured automation and AI agent consumption
- Incremental indexing that watches files and updates only what changed
- Provenance with file path, line numbers, score, and match explanation on every result
- Local embedding model (all-minilm-l6-v2), no API key needed

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/yetone/ctxgrep/main/install.sh | bash
```

Or from source:

```bash
cargo install --git https://github.com/yetone/ctxgrep
```

## Command model

```bash
ctxgrep index <paths...> [options]
ctxgrep search <query> [PATH...] [options]
ctxgrep memory <query> [options]
ctxgrep pack <query> [options]
ctxgrep watch <paths...>
ctxgrep status
ctxgrep clear
ctxgrep doctor
```

## Commands

### index

Build or update the search index.

```bash
ctxgrep index <paths...> [options]
```

Options:

- `-r, --recursive`: recursive scan
- `--hidden`: include hidden files
- `--no-ignore`: don't respect .gitignore
- `--ext <exts>`: file extensions to include (comma-separated)
- `--max-file-size <size>`: max file size (e.g. `5MB`)
- `--rebuild`: rebuild index from scratch
- `--with-memory`: also extract memories during indexing
- `--embedder <provider>`: embedding provider (`local`, `openai`, `none`)
- `--model <name>`: embedding model name

Behavior:

- Incremental by default — only indexes new or changed files.
- `--rebuild` drops existing data and re-indexes everything.
- `--with-memory` runs the heuristic memory extractor during indexing.
- The local embedding model (`all-minilm-l6-v2`, ~86MB ONNX) auto-downloads on first run.

### search

Search indexed documents.

```bash
ctxgrep search <query> [PATH...] [options]
```

Positional arguments:

- `PATH...` (optional): one or more directories or files to search within. Defaults to the current working directory when omitted.

Options:

- `--exact`: literal text match
- `--regex`: regex pattern match
- `--semantic`: vector similarity search
- `--hybrid`: combines lexical + semantic + recency + importance (default)
- `--top-k <n>`: number of results to return (default `10`)
- `--json`: output as JSON
- `--full-section`: show full section content
- `--with-meta`: include metadata in output
- `--path <glob>`: filter by path glob
- `--tag <tag>`: filter by tag
- `--after <date>`: filter results after date
- `--before <date>`: filter results before date
- `--source <type>`: filter by source type
- `--budget <tokens>`: token budget limit
- `--global`: search the entire index across all directories (disables the default CWD scope)

Behavior:

- **Like `grep`, the search scope defaults to the current working directory.** Only documents indexed under that directory are returned.
- Pass explicit `PATH` arguments to search across specific directories instead.
- Use `--global` to search the entire index regardless of where you are.
- Default mode is hybrid, which combines lexical + semantic + recency + importance scoring.
- Use `--exact` only when you know the precise phrase.
- Use `--semantic` for conceptual/meaning-based queries.
- Always use `--json` for agent consumption.

### memory

Search extracted memories (decisions, preferences, constraints, etc.).

```bash
ctxgrep memory <query> [options]
```

Options:

- `--type <TYPE>`: filter by memory type (`fact`, `decision`, `preference`, `definition`, `constraint`, `todo`, `summary`)
- `--json`: output as JSON
- `--top-k <n>`: number of results (default `10`)

Behavior:

- Searches structured memories that were extracted from documents during indexing.
- Use `--type decision` to find past architectural/design decisions before proposing changes.
- Use `--type constraint` to check for restrictions before making changes.

### pack

Assemble a token-budgeted context bundle for a task.

```bash
ctxgrep pack <query> [options]
```

Options:

- `--budget <tokens>`: token budget (default `4000`)
- `--json`: output as JSON
- `--include-memory`: include memory objects
- `--include-snippets`: include text snippets

Behavior:

- Searches, deduplicates, and assembles context within the specified token budget.
- Designed for feeding context into AI agent prompts.

### watch

Watch paths for changes and update index automatically.

```bash
ctxgrep watch <paths...>
```

### status

Show index status (number of documents, chunks, memories, etc.).

```bash
ctxgrep status
```

### clear

Clear all indexed data.

```bash
ctxgrep clear
```

### doctor

Check environment and configuration.

```bash
ctxgrep doctor
```

## JSON output fields

Search results:

- `path` — source file path
- `title` — section title
- `section_path` — hierarchical section path (e.g. `"Brand > Positioning"`)
- `start_line` / `end_line` — line range
- `score` — relevance score (0.0-1.0); below 0.3 is likely noise
- `lexical_score` / `semantic_score` — component scores
- `snippet` — text content
- `why` — match explanation array (e.g. `["title match", "semantic similarity"]`)

Memory results:

- `type` — memory type (fact, decision, preference, etc.)
- `content` — memory text
- `source` — source file path
- `importance` — importance score

## Configuration

Config file: `~/.ctxgrep/config.toml`

Key settings:

- `[embedding] provider`: `"local"` (default), `"openai"`, or `"none"`
- `[retrieval] default_mode`: `"hybrid"` (default)
- `[retrieval] top_k`: default number of results
- `[memory] enabled`: enable/disable memory extraction

## Agent workflow

```bash
# 1. Check index state
ctxgrep status

# 2. Index if needed (only once per corpus)
ctxgrep index ~/project-docs --recursive --with-memory

# 3. Search for context in the current project directory (default CWD scope)
ctxgrep search --json "authentication design"

# 3b. Search the entire index when project scope is too narrow
ctxgrep search --json --global "authentication design"

# 4. Recall specific decisions
ctxgrep memory --json --type decision "authentication"

# 5. Pack context for a task with token budget
ctxgrep pack --json --budget 5000 "implement OAuth2 based on past decisions"
```

## Practical workflows

Broad knowledge search across entire index:

```bash
ctxgrep search --json --global "product positioning history"
```

Search scoped to the current project directory (default):

```bash
cd ~/my-project
ctxgrep search --json "product positioning history"
```

Search across specific directories:

```bash
ctxgrep search --json "product positioning history" ~/notes ~/docs
```

Recall decisions before making changes:

```bash
ctxgrep memory --json --type decision "database schema"
ctxgrep memory --json --type constraint "API versioning"
```

Prepare context for a coding task:

```bash
ctxgrep pack --json --budget 8000 "refactor auth middleware based on past discussions"
```

Search within a specific path glob:

```bash
ctxgrep search --json --path "docs/api/*" "rate limiting"
```

Full-text exact match:

```bash
ctxgrep search --json --exact "Lossless Feedback Loop"
```

Regex pattern search:

```bash
ctxgrep search --json --regex "TODO.*auth"
```

## Guidelines

1. Always use `--json` for structured, parseable output.
2. Use hybrid search (default) for best recall. Use `--exact` only when you know the precise phrase.
3. Use `pack` for task preparation — it deduplicates, categorizes memories, and respects token budgets.
4. Filter results by `score` — discard anything below 0.3.
5. Use `memory --type decision` to find past architectural/design decisions before proposing changes.
6. Index with `--with-memory` to enable memory extraction.
7. Run `ctxgrep doctor` to diagnose issues with the environment or configuration.
8. Like `grep`, search is scoped to the **current working directory** by default. Use explicit `PATH` arguments or `--global` to broaden the scope.
