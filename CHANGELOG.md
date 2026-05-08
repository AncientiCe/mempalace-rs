# Changelog

All notable changes to `mempalace-rs` are documented here.

This Rust implementation uses its own `0.x` version track.

## [0.1.9] - 2026-05-08

### Added

- Agent-memory reliability release focused on fuzzy preference recall, session continuity, and source-grounded MCP search.
- Preference-tagged drawers are now detected on explicit drawer writes and content updates, while preserving unrelated drawer metadata.
- Filter-aware preference recall can supplement hybrid search without crossing requested wing or room boundaries.
- `Palace::search_with_provenance` exposes structured library search results with combined, cosine, BM25, and coding-agent boost scores.
- MCP session context now returns recent diary metadata with project path, topic, timestamp, tags, session ID, and compact text.

### Changed

- MCP search results include clearer score provenance and adjacent source context for agent citation.
- Coding-agent retrieval evals now include additional preference and session-continuity questions for the 0.1.9 reliability lane.

## [0.1.0] - 2026-05-02

### Added

- SQLite-backed drawers, knowledge graph, closets, tunnels, and BM25 metadata.
- Hybrid search combining local vectors and BM25 keyword scoring.
- MCP stdio server, CLI, and Rust `Palace` facade.
- Entity metadata, hall routing, origin detection, i18n language config, and optional LLM refinement hooks.
- Public repository metadata, license, attribution, and release workflow.
- Crates.io package name: `mempalace-rust` (the repository remains `mempalace-rs`).
