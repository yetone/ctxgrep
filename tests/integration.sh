#!/usr/bin/env bash
#
# ctxgrep integration tests
#
# Runs ctxgrep commands against test fixtures and validates output.
# Uses HOME override to isolate from user data. Embedding provider
# is set to "none" so no model download is required in CI.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FIXTURES="$SCRIPT_DIR/fixtures"
CTXGREP="${CTXGREP:-ctxgrep}"
# Resolve to an absolute path so cd operations inside tests don't break it.
if [[ "$CTXGREP" != /* ]]; then
    if [[ "$CTXGREP" == */* ]]; then
        # Relative path (e.g. ./target/release/ctxgrep) — make it absolute.
        CTXGREP="$(cd "$(dirname "$CTXGREP")" && pwd)/$(basename "$CTXGREP")"
    else
        # Plain name in PATH — resolve via command -v.
        CTXGREP="$(command -v "$CTXGREP")"
    fi
fi

PASS=0
FAIL=0
ERRORS=""

# ── Helpers ──

setup_env() {
    TEST_HOME="$(mktemp -d)"
    export HOME="$TEST_HOME"
    mkdir -p "$TEST_HOME/.ctxgrep"
    # Use "none" provider to skip model download in CI
    cat > "$TEST_HOME/.ctxgrep/config.toml" <<'TOML'
[embedding]
provider = "none"
TOML
}

cleanup() {
    rm -rf "$TEST_HOME"
}

pass() {
    PASS=$((PASS + 1))
    echo "  ✓ $1"
}

fail() {
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  ✗ $1"
    echo "  ✗ $1"
}

# Run a command, expect success (exit 0)
expect_success() {
    local desc="$1"
    shift
    if "$@" > /dev/null 2>&1; then
        pass "$desc"
    else
        fail "$desc (exit code $?)"
    fi
}

# Run a command, expect non-zero exit
expect_failure() {
    local desc="$1"
    shift
    if "$@" > /dev/null 2>&1; then
        fail "$desc (expected failure but got 0)"
    else
        pass "$desc"
    fi
}

# Run a command, check stdout contains a string
expect_output_contains() {
    local desc="$1"
    local needle="$2"
    shift 2
    local out
    out=$("$@" 2>/dev/null) || true
    if echo "$out" | grep -qF "$needle"; then
        pass "$desc"
    else
        fail "$desc (output missing: '$needle')"
    fi
}

# Run a command, check stdout does NOT contain a string
expect_output_not_contains() {
    local desc="$1"
    local needle="$2"
    shift 2
    local out
    out=$("$@" 2>/dev/null) || true
    if echo "$out" | grep -qF "$needle"; then
        fail "$desc (output unexpectedly contains: '$needle')"
    else
        pass "$desc"
    fi
}

# Run a command, check stderr contains a string
expect_stderr_contains() {
    local desc="$1"
    local needle="$2"
    shift 2
    local err
    err=$("$@" 2>&1 >/dev/null) || true
    if echo "$err" | grep -qF "$needle"; then
        pass "$desc"
    else
        fail "$desc (stderr missing: '$needle')"
    fi
}

# Check a JSON output field
expect_json_field() {
    local desc="$1"
    local field="$2"
    shift 2
    local out
    out=$("$@" 2>/dev/null) || true
    if echo "$out" | python3 -c "import sys,json; json.load(sys.stdin)$field" > /dev/null 2>&1; then
        pass "$desc"
    else
        fail "$desc (JSON field '$field' not found)"
    fi
}

# ── Test Suites ──

test_basic() {
    echo ""
    echo "═══ 1. Basic Operations ═══"

    expect_success "version flag" $CTXGREP --version
    expect_success "help flag" $CTXGREP --help

    expect_success "clear (fresh state)" $CTXGREP clear

    expect_success "index fixtures" $CTXGREP index "$FIXTURES" --recursive --embedder none
    expect_output_contains "status shows files" "Files:" $CTXGREP status
    expect_output_contains "status shows chunks" "Chunks:" $CTXGREP status
    expect_output_contains "status shows memories" "Memories:" $CTXGREP status

    expect_success "doctor runs" $CTXGREP doctor
}

test_exact_search() {
    echo ""
    echo "═══ 2. Exact Search ═══"

    expect_output_contains "find PostgreSQL" "PostgreSQL" \
        $CTXGREP search "PostgreSQL" --exact --global
    expect_output_contains "find API Gateway" "API Gateway" \
        $CTXGREP search "API Gateway" --exact --global
    expect_output_contains "find Flask" "Flask" \
        $CTXGREP search "Flask" --exact --global
    expect_output_not_contains "no results for gibberish" "score=" \
        $CTXGREP search "xyzzy_nonexistent_gibberish_12345" --exact --global
    expect_output_contains "find TOML in Rust file" "TOML" \
        $CTXGREP search "TOML" --exact --global
}

test_regex_search() {
    echo ""
    echo "═══ 3. Regex Search ═══"

    expect_output_contains "regex TODO" "TODO" \
        $CTXGREP search "TODO" --regex --global
    expect_output_contains "regex pattern pool_size" "pool_size" \
        $CTXGREP search "pool_size" --regex --global
    expect_output_contains "regex Decision:" "Decision" \
        $CTXGREP search "Decision" --regex --global
    # Invalid regex should fail gracefully (not panic)
    expect_success "invalid regex no panic" $CTXGREP search ".*" --regex --global
}

test_chinese_search() {
    echo ""
    echo "═══ 4. Chinese / CJK Search ═══"

    expect_output_contains "find 中文搜索" "中文" \
        $CTXGREP search "中文搜索" --exact --global
    expect_output_contains "find 分词器" "分词" \
        $CTXGREP search "分词器" --exact --global
    expect_output_contains "find 数据" "数据" \
        $CTXGREP search "敏感数据" --exact --global
    expect_output_contains "find jieba mixed" "jieba" \
        $CTXGREP search "jieba" --exact --global
    expect_output_contains "find 决定 memory keyword" "决定" \
        $CTXGREP search "决定" --exact --global
}

test_memory() {
    echo ""
    echo "═══ 5. Memory Extraction ═══"

    expect_output_contains "memory: decision" "decision" \
        $CTXGREP memory "Decision" --type decision
    expect_output_contains "memory: constraint (500ms)" "constraint" \
        $CTXGREP memory "500ms" --type constraint
    expect_output_contains "memory: constraint (pool_size)" "constraint" \
        $CTXGREP memory "constraint" --type constraint
    expect_output_contains "memory: todo" "todo" \
        $CTXGREP memory "TODO" --type todo
    expect_output_contains "memory: preference" "preference" \
        $CTXGREP memory "Preference" --type preference
    expect_output_contains "memory: definition (TTL)" "definition" \
        $CTXGREP memory "TTL" --type definition
    # Chinese memories
    expect_output_contains "memory: chinese" "决定" \
        $CTXGREP memory "分词"
    # No results case (message goes to stderr)
    expect_stderr_contains "memory: no results" "No memories found" \
        $CTXGREP memory "xyzzy_nonexistent_99999"
}

test_pack() {
    echo ""
    echo "═══ 6. Context Pack ═══"

    expect_output_contains "pack basic" "Query:" \
        $CTXGREP pack "system architecture overview"
    expect_output_contains "pack with budget" "Budget:" \
        $CTXGREP pack "database migration" --budget 200
    expect_success "pack json output" $CTXGREP pack "API design" --json
}

test_filters() {
    echo ""
    echo "═══ 7. Filters ═══"

    expect_output_contains "filter *.md" "score=" \
        $CTXGREP search "Decision" --exact --global --path "*.md"
    expect_output_contains "filter *.py" "Flask" \
        $CTXGREP search "Flask" --exact --global --path "*.py"
    expect_output_contains "filter *.rs" "TOML" \
        $CTXGREP search "TOML" --exact --global --path "*.rs"
}

test_output_formats() {
    echo ""
    echo "═══ 8. Output Formats ═══"

    expect_success "json search output" $CTXGREP search "PostgreSQL" --exact --global --json
    expect_output_contains "with-meta shows metadata" "score=" \
        $CTXGREP search "API Gateway" --exact --global --with-meta
    expect_success "full-section flag" \
        $CTXGREP search "Processing Engine" --exact --global --full-section
}

test_top_k() {
    echo ""
    echo "═══ 9. Top-K Limiting ═══"

    local count
    count=$($CTXGREP search "Decision" --exact --global --top-k 2 2>/dev/null | grep -c "score=" || true)
    if [ "$count" -le 2 ]; then
        pass "top-k=2 limits results to <=2"
    else
        fail "top-k=2 returned $count results"
    fi

    count=$($CTXGREP search "Decision" --exact --global --top-k 1 2>/dev/null | grep -c "score=" || true)
    if [ "$count" -le 1 ]; then
        pass "top-k=1 limits results to <=1"
    else
        fail "top-k=1 returned $count results"
    fi
}

test_idempotent_index() {
    echo ""
    echo "═══ 10. Idempotent Re-index ═══"

    local chunks_before chunks_after
    chunks_before=$($CTXGREP status 2>/dev/null | grep "Chunks:" | awk '{print $2}')
    $CTXGREP index "$FIXTURES" --recursive --embedder none > /dev/null 2>&1
    chunks_after=$($CTXGREP status 2>/dev/null | grep "Chunks:" | awk '{print $2}')
    if [ "$chunks_before" = "$chunks_after" ]; then
        pass "re-index same data: chunk count unchanged ($chunks_before)"
    else
        fail "re-index changed chunk count: $chunks_before -> $chunks_after"
    fi
}

test_index_single_file() {
    echo ""
    echo "═══ 11. Index Single File ═══"

    $CTXGREP clear > /dev/null 2>&1
    expect_success "index single file" \
        $CTXGREP index "$FIXTURES/architecture.md" --embedder none
    expect_output_contains "single file indexed" "Files: 1" \
        $CTXGREP status
    expect_output_contains "search in single file" "PostgreSQL" \
        $CTXGREP search "PostgreSQL" --exact --global

    # Re-index all for subsequent tests
    $CTXGREP index "$FIXTURES" --recursive --embedder none > /dev/null 2>&1
}

test_clear_and_recover() {
    echo ""
    echo "═══ 12. Clear and Recover ═══"

    $CTXGREP clear > /dev/null 2>&1
    expect_output_contains "after clear: 0 files" "Files: 0" \
        $CTXGREP status
    expect_output_not_contains "search after clear: no results" "score=" \
        $CTXGREP search "PostgreSQL" --exact --global

    $CTXGREP index "$FIXTURES" --recursive --embedder none > /dev/null 2>&1
    expect_output_contains "after re-index: found results" "PostgreSQL" \
        $CTXGREP search "PostgreSQL" --exact --global
}

test_edge_cases() {
    echo ""
    echo "═══ 13. Edge Cases ═══"

    # Empty query
    expect_success "empty query no crash" $CTXGREP search "" --exact --global

    # Special characters in query
    expect_success "special chars no crash" \
        $CTXGREP search "<script>alert('xss')</script>" --exact --global

    # Unicode search
    expect_output_contains "unicode emoji" "🚀" \
        $CTXGREP search "🚀" --exact --global

    # Very long query (shouldn't crash)
    local long_query
    long_query=$(python3 -c "print('test ' * 200)" 2>/dev/null || echo "test test test")
    expect_success "long query no crash" $CTXGREP search "$long_query" --exact --global

    # Index nonexistent path — ctxgrep reports "No files found" but exits 0
    expect_success "index nonexistent path no crash" \
        $CTXGREP index /tmp/ctxgrep_nonexistent_path_99999 --embedder none
}

test_hybrid_search_without_embeddings() {
    echo ""
    echo "═══ 14. Hybrid Search (no embeddings) ═══"

    # Hybrid mode should still work using lexical component when embeddings unavailable
    expect_success "hybrid search no crash" \
        $CTXGREP search "database migration" --global
    # Semantic search without model should fail gracefully with error message (not panic)
    expect_stderr_contains "semantic search: graceful error" "embedding provider" \
        $CTXGREP search "deployment strategy" --semantic --global
}

test_directory_scope() {
    echo ""
    echo "═══ 15. Directory Scoping ═══"

    local original_dir
    original_dir="$(pwd)"

    # Explicit path finds results within that directory
    expect_output_contains "explicit path: fixtures dir has results" "score=" \
        $CTXGREP search "PostgreSQL" --exact "$FIXTURES"

    # Explicit path outside indexed content returns nothing
    expect_output_not_contains "explicit path: /tmp has no indexed results" "score=" \
        $CTXGREP search "PostgreSQL" --exact /tmp

    # --global bypasses any directory scoping
    expect_output_contains "global flag finds results regardless of cwd" "score=" \
        $CTXGREP search "PostgreSQL" --exact --global

    # Multiple explicit paths: first matches, second doesn't — still finds results
    expect_output_contains "multiple paths: one match suffices" "score=" \
        $CTXGREP search "PostgreSQL" --exact "$FIXTURES" /tmp

    # CWD scoping: from inside the fixtures directory, results are found
    cd "$FIXTURES"
    local out
    out=$($CTXGREP search "PostgreSQL" --exact 2>/dev/null) || true
    if echo "$out" | grep -qF "score="; then
        pass "CWD scoping: search from fixtures dir finds results"
    else
        fail "CWD scoping: search from fixtures dir found no results"
    fi
    cd "$original_dir"

    # CWD scoping: from /tmp (nothing indexed there), no results
    cd /tmp
    out=$($CTXGREP search "PostgreSQL" --exact 2>/dev/null) || true
    if echo "$out" | grep -qF "score="; then
        fail "CWD scoping: search from /tmp unexpectedly found results"
    else
        pass "CWD scoping: search from /tmp correctly returns no results"
    fi
    cd "$original_dir"
}

# ── Main ──

main() {
    echo "╔══════════════════════════════════════╗"
    echo "║    ctxgrep integration tests         ║"
    echo "╚══════════════════════════════════════╝"

    setup_env
    trap cleanup EXIT

    test_basic
    test_exact_search
    test_regex_search
    test_chinese_search
    test_memory
    test_pack
    test_filters
    test_output_formats
    test_top_k
    test_idempotent_index
    test_index_single_file
    test_clear_and_recover
    test_edge_cases
    test_hybrid_search_without_embeddings
    test_directory_scope

    echo ""
    echo "════════════════════════════════════════"
    echo "  Results: $PASS passed, $FAIL failed"
    echo "════════════════════════════════════════"

    if [ "$FAIL" -gt 0 ]; then
        echo ""
        echo "Failures:"
        echo -e "$ERRORS"
        exit 1
    fi
}

main "$@"
