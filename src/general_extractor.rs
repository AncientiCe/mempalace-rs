//! Extract typed memories from text using regex heuristics.
//!
//! Types: decision, preference, milestone, problem, emotional.
//! No LLM required. Port of general_extractor.py.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Decision,
    Preference,
    Milestone,
    Problem,
    Emotional,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::Decision => "decision",
            MemoryType::Preference => "preference",
            MemoryType::Milestone => "milestone",
            MemoryType::Problem => "problem",
            MemoryType::Emotional => "emotional",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub content: String,
    pub memory_type: String,
    pub chunk_index: usize,
}

// ── Marker patterns ────────────────────────────────────────────────────────

static DECISION_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        r"(?i)\blet'?s (use|go with|try|pick|choose|switch to)\b",
        r"(?i)\bwe (should|decided|chose|went with|picked|settled on)\b",
        r"(?i)\bi'?m going (to|with)\b",
        r"(?i)\bbetter (to|than|approach|option|choice)\b",
        r"(?i)\binstead of\b",
        r"(?i)\brather than\b",
        r"(?i)\bthe reason (is|was|being)\b",
        r"(?i)\bbecause\b",
        r"(?i)\btrade-?off\b",
        r"(?i)\bpros and cons\b",
        r"(?i)\barchitecture\b",
        r"(?i)\bapproach\b",
        r"(?i)\bstrategy\b",
        r"(?i)\bpattern\b",
        r"(?i)\bstack\b",
        r"(?i)\bframework\b",
        r"(?i)\binfrastructure\b",
        r"(?i)\bset (it |this )?to\b",
        r"(?i)\bconfigure\b",
        r"(?i)\bdefault\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static PREFERENCE_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        r"(?i)\bi prefer\b",
        r"(?i)\balways use\b",
        r"(?i)\bnever use\b",
        r"(?i)\bdon'?t (ever |like to )?(use|do|mock|stub|import)\b",
        r"(?i)\bi like (to|when|how)\b",
        r"(?i)\bi hate (when|how|it when)\b",
        r"(?i)\bplease (always|never|don'?t)\b",
        r"(?i)\bmy (rule|preference|style|convention) is\b",
        r"(?i)\bwe (always|never)\b",
        r"(?i)\bsnake_?case\b",
        r"(?i)\bcamel_?case\b",
        r"(?i)\buse\b.*\binstead of\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static MILESTONE_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        r"(?i)\bit works\b",
        r"(?i)\bit worked\b",
        r"(?i)\bgot it working\b",
        r"(?i)\bfixed\b",
        r"(?i)\bsolved\b",
        r"(?i)\bbreakthrough\b",
        r"(?i)\bfigured (it )?out\b",
        r"(?i)\bnailed it\b",
        r"(?i)\bfinally\b",
        r"(?i)\bfirst time\b",
        r"(?i)\bdiscovered\b",
        r"(?i)\brealized\b",
        r"(?i)\bturns out\b",
        r"(?i)\bthe key (is|was|insight)\b",
        r"(?i)\bthe trick (is|was)\b",
        r"(?i)\bnow i (understand|see|get it)\b",
        r"(?i)\bbuilt\b",
        r"(?i)\bcreated\b",
        r"(?i)\bimplemented\b",
        r"(?i)\bshipped\b",
        r"(?i)\blaunched\b",
        r"(?i)\bdeployed\b",
        r"(?i)\breleased\b",
        r"(?i)\bprototype\b",
        r"(?i)\bproof of concept\b",
        r"(?i)\bdemo\b",
        r"(?i)\bversion \d",
        r"(?i)\bv\d+\.\d+",
        r"(?i)\d+x (compression|faster|slower|better|improvement|reduction)",
        r"(?i)\d+% (reduction|improvement|faster|better|smaller)",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static PROBLEM_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        r"(?i)\b(bug|error|crash|fail|broke|broken|issue|problem)\b",
        r"(?i)\bdoesn'?t work\b",
        r"(?i)\bnot working\b",
        r"(?i)\bwon'?t\b.*\bwork\b",
        r"(?i)\bkeeps? (failing|crashing|breaking|erroring)\b",
        r"(?i)\broot cause\b",
        r"(?i)\bthe (problem|issue|bug) (is|was)\b",
        r"(?i)\bturns out\b.*\b(was|because|due to)\b",
        r"(?i)\bthe fix (is|was)\b",
        r"(?i)\bworkaround\b",
        r"(?i)\bthat'?s why\b",
        r"(?i)\bthe reason it\b",
        r"(?i)\bfixed (it |the |by )\b",
        r"(?i)\bsolution (is|was)\b",
        r"(?i)\bresolved\b",
        r"(?i)\bpatched\b",
        r"(?i)\bthe answer (is|was)\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static EMOTION_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        r"(?i)\blove\b",
        r"(?i)\bscared\b",
        r"(?i)\bafraid\b",
        r"(?i)\bproud\b",
        r"(?i)\bhurt\b",
        r"(?i)\bhappy\b",
        r"(?i)\bsad\b",
        r"(?i)\bcry\b",
        r"(?i)\bcrying\b",
        r"(?i)\bmiss\b",
        r"(?i)\bsorry\b",
        r"(?i)\bgrateful\b",
        r"(?i)\bangry\b",
        r"(?i)\bworried\b",
        r"(?i)\blonely\b",
        r"(?i)\bbeautiful\b",
        r"(?i)\bamazing\b",
        r"(?i)\bwonderful\b",
        r"(?i)i feel",
        r"(?i)i'm scared",
        r"(?i)i love you",
        r"(?i)i'm sorry",
        r"(?i)i can't",
        r"(?i)i wish",
        r"(?i)i miss",
        r"(?i)i need",
        r"(?i)never told anyone",
        r"(?i)nobody knows",
        r"\*[^*]+\*",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static POSITIVE_WORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "pride",
        "proud",
        "joy",
        "happy",
        "love",
        "loving",
        "beautiful",
        "amazing",
        "wonderful",
        "incredible",
        "fantastic",
        "brilliant",
        "perfect",
        "excited",
        "thrilled",
        "grateful",
        "warm",
        "breakthrough",
        "success",
        "works",
        "working",
        "solved",
        "fixed",
        "nailed",
        "heart",
        "hug",
        "precious",
        "adore",
    ]
    .iter()
    .copied()
    .collect()
});

static NEGATIVE_WORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "bug",
        "error",
        "crash",
        "crashing",
        "crashed",
        "fail",
        "failed",
        "failing",
        "failure",
        "broken",
        "broke",
        "breaking",
        "breaks",
        "issue",
        "problem",
        "wrong",
        "stuck",
        "blocked",
        "unable",
        "impossible",
        "missing",
        "terrible",
        "horrible",
        "awful",
        "worse",
        "worst",
        "panic",
        "disaster",
        "mess",
    ]
    .iter()
    .copied()
    .collect()
});

static CODE_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        r"^\s*[\$#]\s",
        r"^\s*(cd|source|echo|export|pip|npm|git|python|bash|curl|wget|mkdir|rm|cp|mv|ls|cat|grep|find|chmod|sudo|brew|docker)\s",
        r"^\s*```",
        r"^\s*(import|from|def|class|function|const|let|var|return)\s",
        r"^\s*[A-Z_]{2,}=",
        r"^\s*\|",
        r"^\s*[-]{2,}",
        r"^\s*[{}\[\]]\s*$",
        r"^\s*(if|for|while|try|except|elif|else:)\b",
        r"^\s*\w+\.\w+\(",
        r"^\s*\w+ = \w+\.\w+",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static TURN_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"^>\s").unwrap(),
        Regex::new(r"(?i)^(Human|User|Q)\s*:").unwrap(),
        Regex::new(r"(?i)^(Assistant|AI|A|Claude|ChatGPT)\s*:").unwrap(),
    ]
});

fn score_markers(text: &str, pats: &[Regex]) -> f64 {
    pats.iter().map(|p| p.find_iter(text).count() as f64).sum()
}

fn is_code_line(line: &str) -> bool {
    let stripped = line.trim();
    if stripped.is_empty() {
        return false;
    }
    for pat in CODE_PATS.iter() {
        if pat.is_match(stripped) {
            return true;
        }
    }
    let alpha_ratio = stripped.chars().filter(|c| c.is_alphabetic()).count() as f64
        / stripped.len().max(1) as f64;
    alpha_ratio < 0.4 && stripped.len() > 10
}

fn extract_prose(text: &str) -> String {
    let mut prose = Vec::new();
    let mut in_code = false;
    for line in text.lines() {
        if line.trim().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        if !is_code_line(line) {
            prose.push(line);
        }
    }
    let result = prose.join("\n").trim().to_string();
    if result.is_empty() {
        text.to_string()
    } else {
        result
    }
}

fn get_sentiment(text: &str) -> i32 {
    let words: HashSet<&str> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .collect();
    let pos = words
        .iter()
        .filter(|w| POSITIVE_WORDS.contains(**w))
        .count() as i32;
    let neg = words
        .iter()
        .filter(|w| NEGATIVE_WORDS.contains(**w))
        .count() as i32;
    pos - neg
}

fn has_resolution(text: &str) -> bool {
    static RESOLUTION_PATS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            r"(?i)\bfixed\b",
            r"(?i)\bsolved\b",
            r"(?i)\bresolved\b",
            r"(?i)\bpatched\b",
            r"(?i)\bgot it working\b",
            r"(?i)\bit works\b",
            r"(?i)\bnailed it\b",
            r"(?i)\bfigured (it )?out\b",
            r"(?i)\bthe (fix|answer|solution)\b",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
    });
    RESOLUTION_PATS.iter().any(|p| p.is_match(text))
}

fn split_into_segments(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let turn_count = lines
        .iter()
        .filter(|l| TURN_PATS.iter().any(|p| p.is_match(l.trim())))
        .count();

    if turn_count >= 3 {
        return split_by_turns(&lines);
    }

    let paragraphs: Vec<String> = text
        .split("\n\n")
        .filter_map(|p| {
            let s = p.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
        .collect();

    if paragraphs.len() <= 1 && lines.len() > 20 {
        return lines
            .chunks(25)
            .map(|chunk| chunk.join("\n").trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    paragraphs
}

fn split_by_turns(lines: &[&str]) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();

    for line in lines {
        let is_turn = TURN_PATS.iter().any(|p| p.is_match(line.trim()));
        if is_turn && !current.is_empty() {
            segments.push(current.join("\n"));
            current = vec![line];
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        segments.push(current.join("\n"));
    }
    segments
}

/// Extract typed memories from text.
pub fn extract_memories(text: &str, min_confidence: f64) -> Vec<Memory> {
    let paragraphs = split_into_segments(text);
    let mut memories = Vec::new();

    for para in &paragraphs {
        if para.trim().len() < 20 {
            continue;
        }
        let prose = extract_prose(para);

        let scores: [(MemoryType, f64); 5] = [
            (MemoryType::Decision, score_markers(&prose, &DECISION_PATS)),
            (
                MemoryType::Preference,
                score_markers(&prose, &PREFERENCE_PATS),
            ),
            (
                MemoryType::Milestone,
                score_markers(&prose, &MILESTONE_PATS),
            ),
            (MemoryType::Problem, score_markers(&prose, &PROBLEM_PATS)),
            (MemoryType::Emotional, score_markers(&prose, &EMOTION_PATS)),
        ];

        let nonzero: Vec<(&MemoryType, f64)> = scores
            .iter()
            .filter(|(_, s)| *s > 0.0)
            .map(|(t, s)| (t, *s))
            .collect();
        if nonzero.is_empty() {
            continue;
        }

        let length_bonus = if para.len() > 500 {
            2.0
        } else if para.len() > 200 {
            1.0
        } else {
            0.0
        };
        let (best_type, best_raw) = nonzero
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        let best_score = best_raw + length_bonus;

        // Disambiguate
        let sentiment = get_sentiment(&prose);
        let effective_type: MemoryType = match best_type {
            MemoryType::Problem if has_resolution(&prose) => {
                if scores[4].1 > 0.0 && sentiment > 0 {
                    MemoryType::Emotional
                } else {
                    MemoryType::Milestone
                }
            }
            MemoryType::Problem if sentiment > 0 => {
                if scores[2].1 > 0.0 {
                    MemoryType::Milestone
                } else if scores[4].1 > 0.0 {
                    MemoryType::Emotional
                } else {
                    MemoryType::Problem
                }
            }
            other => (*other).clone(),
        };

        let confidence = (best_score / 5.0).min(1.0);
        if confidence < min_confidence {
            continue;
        }

        memories.push(Memory {
            content: para.trim().to_string(),
            memory_type: effective_type.as_str().to_string(),
            chunk_index: memories.len(),
        });
    }

    memories
}
