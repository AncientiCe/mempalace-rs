//! AAAK dialect — compressed memory format for MemPalace.
//!
//! Contains the AAAK spec and PALACE_PROTOCOL constants used in MCP status responses.
//! Also provides token counting and basic compression stats. Port of dialect.py.

/// Protocol instructions embedded in the MCP status response.
pub const PALACE_PROTOCOL: &str = "IMPORTANT — MemPalace Memory Protocol:
1. ON WAKE-UP: Call mempalace_status to load palace overview + AAAK spec.
2. BEFORE RESPONDING about any person, project, or past event: call mempalace_kg_query or mempalace_search FIRST. Never guess — verify.
3. IF UNSURE about a fact (name, gender, age, relationship): say \"let me check\" and query the palace. Wrong is worse than slow.
4. AFTER EACH SESSION: call mempalace_diary_write to record what happened, what you learned, what matters.
5. WHEN FACTS CHANGE: call mempalace_kg_invalidate on the old fact, mempalace_kg_add for the new one.

This protocol ensures the AI KNOWS before it speaks. Storage is not memory — but storage + this protocol = memory.";

/// The AAAK compressed memory dialect specification.
pub const AAAK_SPEC: &str = "AAAK is a compressed memory dialect that MemPalace uses for efficient storage.
It is designed to be readable by both humans and LLMs without decoding.

FORMAT:
  ENTITIES: 3-letter uppercase codes. ALC=Alice, JOR=Jordan, RIL=Riley, MAX=Max, BEN=Ben.
  EMOTIONS: *action markers* before/during text. *warm*=joy, *fierce*=determined, *raw*=vulnerable, *bloom*=tenderness.
  STRUCTURE: Pipe-separated fields. FAM: family | PROJ: projects | ⚠: warnings/reminders.
  DATES: ISO format (2026-03-31). COUNTS: Nx = N mentions (e.g., 570x).
  IMPORTANCE: ★ to ★★★★★ (1-5 scale).
  HALLS: hall_facts, hall_events, hall_discoveries, hall_preferences, hall_advice.
  WINGS: wing_user, wing_agent, wing_team, wing_code, wing_myproject, wing_hardware, wing_ue5, wing_ai_research.
  ROOMS: Hyphenated slugs representing named ideas (e.g., chromadb-setup, gpu-pricing).

EXAMPLE:
  FAM: ALC→♡JOR | 2D(kids): RIL(18,sports) MAX(11,chess+swimming) | BEN(contributor)

Read AAAK naturally — expand codes mentally, treat *markers* as emotional context.
When WRITING AAAK: use entity codes, mark emotions, keep structure tight.";

/// Rough token estimate: ~4 chars per token (same heuristic as Python version).
pub fn token_count(text: &str) -> usize {
    text.len() / 4
}

/// Heuristic AAAK-style compression: strip vowels from common words, abbreviate.
/// This is a lightweight approximation — real AAAK is written by the AI, not generated.
pub fn compress(text: &str) -> String {
    // For now return the text with a note — actual AAAK is AI-authored
    format!("[AAAK] {text}")
}

/// Basic compression statistics.
pub fn compression_stats(original: &str, compressed: &str) -> serde_json::Value {
    let original_tokens = token_count(original);
    let compressed_tokens = token_count(compressed);
    let ratio = if original_tokens > 0 {
        compressed_tokens as f64 / original_tokens as f64
    } else {
        1.0
    };
    serde_json::json!({
        "original_tokens": original_tokens,
        "compressed_tokens": compressed_tokens,
        "compression_ratio": (ratio * 1000.0).round() / 1000.0,
        "savings_pct": ((1.0 - ratio) * 100.0).round() as i64,
    })
}
