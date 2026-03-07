# ctxgrep

grep for context, not just text.

ctxgrep is a local-first CLI for searching documents, notes, memories, and project context. It combines exact search, regex, semantic retrieval, and memory extraction to help humans and AI agents pull the right context into the current task.

## Give Your AI Agent Long-Term Memory

Paste the following into Claude Code or any coding agent that supports tool use:

```
Install ctxgrep (a local-first context & memory retrieval CLI) and add its
agent skill so you can search my docs, recall past decisions, and pack context:

curl -fsSL https://raw.githubusercontent.com/yetone/ctxgrep/main/install.sh | bash

Then install the skill:
claude skill add --url https://github.com/yetone/ctxgrep/raw/main/skill/ctxgrep.skill

After that, index my project docs:
ctxgrep index ~/notes ~/docs --recursive

No API key needed — it ships with a local embedding model.
```

That's it. Your agent can now `ctxgrep search`, `ctxgrep memory`, and `ctxgrep pack` to pull relevant context into any task.

## Features

- **grep-like CLI** — familiar command-line interface
- **Local-first indexing** — SQLite + FTS5, no server needed
- **Hybrid retrieval** — exact / regex / semantic / hybrid search modes
- **Memory extraction** — automatically extracts decisions, preferences, constraints from text
- **Context packing** — assembles a token-budgeted context bundle for agents
- **JSON output** — structured output for automation and AI agents
- **Incremental indexing** — watches files and updates only what changed
- **Provenance** — every result includes file path, line numbers, score, and why it matched

## Installation

### From binary releases

```bash
curl -fsSL https://raw.githubusercontent.com/yetone/ctxgrep/main/install.sh | bash
```

### From source

```bash
cargo install --git https://github.com/yetone/ctxgrep
```

### Build from source

```bash
git clone https://github.com/yetone/ctxgrep
cd ctxgrep
cargo build --release
# Binary at target/release/ctxgrep
```

## Quick Start

```bash
# Index your documents
ctxgrep index ~/notes ~/docs ~/meetings --recursive

# Exact search
ctxgrep search --exact "Lossless Feedback Loop"

# Semantic search (local model, no API key needed)
ctxgrep search --semantic "how did we define serious coding"

# Hybrid search (default, combines lexical + semantic)
ctxgrep search "product positioning history"

# Search extracted memories
ctxgrep memory "naming preferences"
ctxgrep memory --type decision "product positioning"

# Pack context for a task
ctxgrep pack --budget 4000 "prepare context for the Serious Coding talk"

# Watch for changes
ctxgrep watch ~/notes ~/docs

# Check status
ctxgrep status

# Check environment
ctxgrep doctor
```

## Commands

| Command | Description |
|---------|-------------|
| `ctxgrep index <paths...>` | Build or update index |
| `ctxgrep search <query>` | Search indexed documents |
| `ctxgrep memory <query>` | Search extracted memories |
| `ctxgrep pack <query>` | Pack context for a task |
| `ctxgrep watch <paths...>` | Watch paths for changes |
| `ctxgrep status` | Show index status |
| `ctxgrep clear` | Clear all indexed data |
| `ctxgrep doctor` | Check environment |

## Search Modes

- `--exact` — literal text match
- `--regex` — regex pattern match
- `--semantic` — vector similarity search (requires embedding provider)
- `--hybrid` — combines lexical + semantic + recency + importance (default)

## Output

### Human-readable (default)

```
~/notes/brand/serious-coding.md:42-68  [score=0.91]
  Serious Coding
  Serious Coding is defined as...
  why: title match, semantic similarity, contains key phrase
```

### JSON (`--json`)

```json
{
  "query": "product positioning",
  "mode": "hybrid",
  "results": [
    {
      "chunk_id": "...",
      "path": "/Users/you/notes/brand/yansu.md",
      "title": "Yansu Positioning",
      "section_path": "Brand > Positioning",
      "start_line": 42,
      "end_line": 68,
      "score": 0.91,
      "lexical_score": 0.74,
      "semantic_score": 0.88,
      "snippet": "Yansu should be framed as Serious Coding...",
      "why": ["title match", "semantic similarity"]
    }
  ]
}
```

## Configuration

Config file: `~/.ctxgrep/config.toml`

```toml
[index]
db_path = "~/.ctxgrep/index.db"
follow_gitignore = true
max_file_size = "5MB"
default_extensions = ["md", "txt", "org", "rst", "jsonl", "json", "py", "ts", "js", "go", "rs", "lua"]

[embedding]
provider = "local"            # "local", "openai", or "none"
model = "all-minilm-l6-v2"   # local ONNX model, auto-downloaded
dimensions = 384

[retrieval]
default_mode = "hybrid"
top_k = 10
semantic_weight = 0.35
lexical_weight = 0.35
recency_weight = 0.15
importance_weight = 0.10
scope_weight = 0.05

[memory]
enabled = true
extractor = "heuristic"
min_importance = 0.60

[pack]
default_budget = 4000
include_sources = true
include_snippets = true
```

## Semantic Search

Semantic search uses a local embedding model by default (`all-minilm-l6-v2`, ~86MB ONNX). The model auto-downloads on first run — no API key needed.

Alternatively, set `provider = "openai"` in config and export `OPENAI_API_KEY` to use OpenAI embeddings.

## Memory Types

ctxgrep automatically extracts structured memories from your documents:

| Type | Description |
|------|-------------|
| `fact` | Stable facts, conventions, standards |
| `decision` | Decisions that were made |
| `preference` | User preferences and habits |
| `definition` | Term definitions |
| `constraint` | Restrictions and limitations |
| `todo` | Action items |
| `summary` | Summaries and conclusions |

## Supported File Formats

`.md` `.txt` `.org` `.rst` `.json` `.jsonl` `.py` `.ts` `.js` `.go` `.rs` `.lua` and more.

## Data Storage

All data is stored locally in `~/.ctxgrep/`:

- `index.db` — SQLite database with FTS5
- `config.toml` — configuration
- `cache/` — temporary cache

## License

MIT
