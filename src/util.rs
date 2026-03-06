use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn estimate_tokens(text: &str) -> usize {
    // Rough: ~4 chars per token for English, CJK chars ≈ 1 token each
    let mut ascii_chars = 0usize;
    let mut cjk_chars = 0usize;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii_chars += 1;
        } else {
            cjk_chars += 1;
        }
    }
    ascii_chars.div_ceil(4) + cjk_chars
}

pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let t: String = s.chars().take(max_chars).collect();
        format!("{t}...")
    }
}

pub fn detect_language(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "js" => Some("javascript"),
        "ts" => Some("typescript"),
        "go" => Some("go"),
        "lua" => Some("lua"),
        "rb" => Some("ruby"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        "md" => Some("markdown"),
        "txt" => Some("text"),
        "org" => Some("org"),
        "rst" => Some("rst"),
        "json" => Some("json"),
        "jsonl" => Some("jsonl"),
        _ => None,
    }
    .map(String::from)
}

pub fn mime_from_ext(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    match ext {
        "md" => Some("text/markdown"),
        "txt" => Some("text/plain"),
        "org" => Some("text/org"),
        "rst" => Some("text/x-rst"),
        "json" | "jsonl" => Some("application/json"),
        _ => Some("text/plain"),
    }
    .map(String::from)
}
