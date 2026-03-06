use serde::{Deserialize, Serialize};

// ── File ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub file_id: String,
    pub path: String,
    pub source_type: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub content_hash: String,
    pub size_bytes: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub indexed_at: String,
    pub tags: Vec<String>,
}

// ── Chunk ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    HeadingSection,
    ParagraphBlock,
    ListBlock,
    QuoteBlock,
    CodeBlock,
    ChatTurn,
    TranscriptSegment,
    GenericWindow,
}

impl ChunkType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::HeadingSection => "heading_section",
            Self::ParagraphBlock => "paragraph_block",
            Self::ListBlock => "list_block",
            Self::QuoteBlock => "quote_block",
            Self::CodeBlock => "code_block",
            Self::ChatTurn => "chat_turn",
            Self::TranscriptSegment => "transcript_segment",
            Self::GenericWindow => "generic_window",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "heading_section" => Self::HeadingSection,
            "paragraph_block" => Self::ParagraphBlock,
            "list_block" => Self::ListBlock,
            "quote_block" => Self::QuoteBlock,
            "code_block" => Self::CodeBlock,
            "chat_turn" => Self::ChatTurn,
            "transcript_segment" => Self::TranscriptSegment,
            _ => Self::GenericWindow,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub chunk_id: String,
    pub file_id: String,
    pub doc_id: Option<String>,
    pub title: Option<String>,
    pub section_path: Option<String>,
    pub chunk_type: ChunkType,
    pub ordinal: i32,
    pub content: String,
    pub content_preview: Option<String>,
    pub token_count: i32,
    pub start_line: i32,
    pub end_line: i32,
    pub heading_level: Option<i32>,
    pub speaker: Option<String>,
    pub timestamp: Option<String>,
    pub tags: Vec<String>,
}

// ── Memory ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Fact,
    Decision,
    Preference,
    Definition,
    Constraint,
    Todo,
    Summary,
}

impl MemoryType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Preference => "preference",
            Self::Definition => "definition",
            Self::Constraint => "constraint",
            Self::Todo => "todo",
            Self::Summary => "summary",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fact" => Some(Self::Fact),
            "decision" => Some(Self::Decision),
            "preference" => Some(Self::Preference),
            "definition" => Some(Self::Definition),
            "constraint" => Some(Self::Constraint),
            "todo" => Some(Self::Todo),
            "summary" => Some(Self::Summary),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub memory_id: String,
    pub source_chunk_id: String,
    pub memory_type: MemoryType,
    pub subject: String,
    pub normalized_subject: Option<String>,
    pub content: String,
    pub importance: f64,
    pub confidence: f64,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub extracted_at: String,
    pub tags: Vec<String>,
}

// ── Search Result ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultType {
    Chunk,
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    pub chunk_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_path: Option<String>,
    pub start_line: i32,
    pub end_line: i32,
    pub score: f64,
    pub lexical_score: f64,
    pub semantic_score: f64,
    pub recency_score: f64,
    pub importance_score: f64,
    pub snippet: String,
    pub why: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub result_type: ResultType,
}

// ── Pack Result ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackResult {
    pub query: String,
    pub budget_tokens: usize,
    pub summary: String,
    pub facts: Vec<String>,
    pub decisions: Vec<String>,
    pub preferences: Vec<String>,
    pub constraints: Vec<String>,
    pub open_questions: Vec<String>,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: String,
    pub chunk_id: String,
    pub score: f64,
}

// ── Index Stats ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_files: i64,
    pub total_chunks: i64,
    pub total_memories: i64,
    pub total_embeddings: i64,
    pub embedding_model: String,
    pub db_path: String,
    pub last_indexed_at: Option<String>,
}

// ── Search Options ──

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchMode {
    Exact,
    Regex,
    Semantic,
    Hybrid,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub top_k: usize,
    pub path_filter: Option<String>,
    pub tag_filter: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
}
