use crate::model::*;
use crate::util;

/// Extract memory objects from a chunk using heuristic rules.
pub fn extract_memories(chunk: &Chunk) -> Vec<Memory> {
    let mut memories = Vec::new();
    let sentences = split_sentences(&chunk.content);

    for sentence in &sentences {
        let trimmed = sentence.trim();
        if trimmed.len() < 8 {
            continue;
        }

        if let Some((mem_type, importance, confidence)) = classify_sentence(trimmed) {
            let subject = extract_subject(trimmed);
            memories.push(Memory {
                memory_id: util::generate_id(),
                source_chunk_id: chunk.chunk_id.clone(),
                memory_type: mem_type,
                subject: subject.clone(),
                normalized_subject: Some(normalize_subject(&subject)),
                content: trimmed.to_string(),
                importance,
                confidence,
                valid_from: None,
                valid_to: None,
                extracted_at: util::now_iso(),
                tags: Vec::new(),
            });
        }
    }

    memories
}

// ── Sentence splitting ──

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        // Split on Chinese/English sentence terminators
        if matches!(ch, '.' | '!' | '?' | '。' | '！' | '？' | '\n') {
            let s = current.trim().to_string();
            if !s.is_empty() {
                sentences.push(s);
            }
            current.clear();
        }
    }
    let s = current.trim().to_string();
    if !s.is_empty() {
        sentences.push(s);
    }
    sentences
}

// ── Classification ──

struct Signal {
    patterns: &'static [&'static str],
    mem_type: MemoryType,
    importance: f64,
    confidence: f64,
}

const SIGNALS: &[Signal] = &[
    Signal {
        patterns: &[
            "决定",
            "decided",
            "we decided",
            "decision:",
            "决策",
            "确定",
            "最终选择",
            "final decision",
        ],
        mem_type: MemoryType::Decision,
        importance: 0.85,
        confidence: 0.80,
    },
    Signal {
        patterns: &[
            "偏好",
            "prefer",
            "preference",
            "喜欢用",
            "习惯用",
            "倾向于",
            "我更喜欢",
            "i prefer",
            "always use",
        ],
        mem_type: MemoryType::Preference,
        importance: 0.75,
        confidence: 0.75,
    },
    Signal {
        patterns: &[
            "定义为",
            "defined as",
            "definition:",
            "定义是",
            "是指",
            "means",
            "refers to",
            "术语",
        ],
        mem_type: MemoryType::Definition,
        importance: 0.80,
        confidence: 0.85,
    },
    Signal {
        patterns: &[
            "约束",
            "constraint",
            "不能",
            "不要",
            "禁止",
            "must not",
            "never",
            "不允许",
            "限制",
            "restriction",
            "不可以",
        ],
        mem_type: MemoryType::Constraint,
        importance: 0.85,
        confidence: 0.80,
    },
    Signal {
        patterns: &[
            "TODO",
            "todo",
            "待办",
            "需要做",
            "要记得",
            "别忘了",
            "FIXME",
            "HACK",
            "记得",
        ],
        mem_type: MemoryType::Todo,
        importance: 0.70,
        confidence: 0.70,
    },
    Signal {
        patterns: &[
            "结论是",
            "总结",
            "summary:",
            "in summary",
            "综上",
            "总的来说",
            "to summarize",
            "conclusion",
        ],
        mem_type: MemoryType::Summary,
        importance: 0.75,
        confidence: 0.80,
    },
    Signal {
        patterns: &[
            "以后统一",
            "统一用",
            "规定",
            "标准是",
            "convention",
            "standard:",
            "规范",
            "rule:",
            "原则是",
        ],
        mem_type: MemoryType::Fact,
        importance: 0.80,
        confidence: 0.80,
    },
];

fn classify_sentence(sentence: &str) -> Option<(MemoryType, f64, f64)> {
    let lower = sentence.to_lowercase();
    for signal in SIGNALS {
        for pattern in signal.patterns {
            if lower.contains(&pattern.to_lowercase()) {
                return Some((
                    signal.mem_type.clone(),
                    signal.importance,
                    signal.confidence,
                ));
            }
        }
    }
    None
}

// ── Subject extraction ──

fn extract_subject(sentence: &str) -> String {
    // Simple heuristic: take the first meaningful clause (up to 60 chars)
    let s = sentence
        .trim()
        .trim_start_matches(|c: char| c == '-' || c == '*' || c == '>' || c.is_whitespace());
    let end = s
        .find([',', '，', ':', '：', ';', '；', '(', '（'])
        .unwrap_or(s.len().min(60));
    s[..end].trim().to_string()
}

fn normalize_subject(subject: &str) -> String {
    subject
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c > '\u{4e00}')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
