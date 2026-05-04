# mempalace-rs

[![CI](https://github.com/AncientiCe/mempalace-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/AncientiCe/mempalace-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.82+](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org)

A local memory palace for AI assistants, implemented in Rust.

This project stores verbatim text, embeds it locally, and retrieves relevant
drawers with semantic search. It can be used as a CLI, an MCP stdio server, or
as a Rust library through the `Palace` facade.

## What It Does

- Stores project files and conversation turns in a local SQLite database.
- Generates local embeddings with ONNX Runtime and `all-MiniLM-L6-v2`.
- Retrieves memories by semantic similarity with optional wing/room filters.
- Provides a knowledge graph for temporal entity relationships.
- Exposes MCP tools for assistants that support Model Context Protocol.
- Offers a small Rust library API for embedding memory into other services.

## Storage

Collapses Python's dual-store (ChromaDB + SQLite) into **one file** at `~/.mempalace/palace.db`:

| Table | Purpose |
|---|---|
| `drawers` | Text content + embedding BLOB + metadata |
| `entities` | KG entity nodes |
| `triples` | KG temporal relationship edges |

Embeddings are stored as `f32` vectors from `all-MiniLM-L6-v2`. Search uses local cosine similarity over the stored vectors.

---

## Benchmarks

Retrieval recall on the LongMemEval `s_cleaned` split — 500 questions over conversational haystacks of ~50 sessions / ~115k tokens each (30 abstention questions are filtered out per the standard convention, leaving 470 evaluated).

The recipe behind the numbers below:

- **Granularity**: one drawer per session.
- **Indexed content**: the **full session** — both user and assistant turns are stored and embedded together. No user-turn filtering, no summarization, no LLM extraction.
- **Embedder**: `all-MiniLM-L6-v2` (384-dim, ONNX), 512-token cap, run locally — no API calls.
- **Retrieval**: **hybrid** — BM25 (k1=1.5, b=0.75, weight 0.35) fused with cosine similarity (weight 0.65), top-K = 10. Pure score fusion, no hand-tuned keyword/temporal/preference boosters.
- **No LLM at any stage**: no extraction, no rerank, no answer generation. The recall numbers measure the retriever in isolation.
- **Metric**: `recall_any@K` at session granularity — does any gold session appear in the top-K results?
- **Hardware**: Apple M1 Pro, 10 cores (8P + 2E), 32 GB RAM.

| Split | R@1 | R@5 | R@10 |
|---|---:|---:|---:|
| `longmemeval_oracle` (sanity check) | 1.000 | 1.000 | 1.000 |
| `longmemeval_s_cleaned` | **0.889** | **0.981** | **0.991** |

Per-question-type on `s_cleaned`:

| Question type | R@1 | R@5 | R@10 |
|---|---:|---:|---:|
| knowledge-update | 0.944 | 1.000 | 1.000 |
| multi-session | 0.909 | 0.983 | 1.000 |
| single-session-assistant | 1.000 | 1.000 | 1.000 |
| single-session-preference | 0.633 | 0.867 | 0.933 |
| single-session-user | 0.922 | 1.000 | 1.000 |
| temporal-reasoning | 0.835 | 0.976 | 0.984 |

### Reading the numbers

- **`oracle` is a sanity check, not a real result.** That split hands the retriever only the sessions known to contain the answer, so perfect recall just confirms the pipeline is wired up correctly.
- **`s_cleaned` is the real test.** ~50 sessions / ~115k tokens of conversational haystack per question, no hints. R@5 = 0.981 means that for 461 of 470 evaluated questions, a gold session appears somewhere in the top 5 retrieved.
- **R@1 → R@5 → R@10 tells you where the failures cluster.** The jump from 0.889 to 0.981 means most "misses" at top-1 are near-misses — the right session is usually rank 2–5, displaced by a lexically similar distractor. The further jump to 0.991 at top-10 means only ~9 questions out of 470 fall outside the top-10 entirely; those are the genuinely hard cases.
- **Per-question-type breakdown is where the model's blind spots show.**
  - `single-session-assistant`, `single-session-user`, `knowledge-update`: ≥0.94 at R@1, ≈1.0 at R@5. The retriever handles direct questions where the answer is stated verbatim in one session.
  - `multi-session` and `temporal-reasoning`: strong at R@5 (~0.98) but lower at R@1 (~0.83–0.91). Multiple sessions are relevant and the "best" one is a judgement call — top-1 ranking among near-equivalents is genuinely ambiguous.
  - `single-session-preference`: the visible weak spot at 0.633 / 0.867 / 0.933. Preference questions ("what's my favorite X") are answered by sentences like *"I like…"* / *"I prefer…"* that don't share keywords with the question. Pure BM25 + frozen MiniLM has no signal for preference-shaped sentences specifically; closing this gap would require either an LLM-extracted preference index or a hand-rolled pattern booster.
- **What's deliberately *not* in these numbers.** No LLM at any stage — no extraction during ingest, no query rewriting, no rerank, no answer generation. No per-dataset hyperparameter tuning. No keyword/temporal/preference boosters. No GPU. The result is the retrieval engine in isolation, on a single CPU, with fixed defaults.

---

## Installation

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/AncientiCe/mempalace-rs/main/scripts/install.sh | sh
```

### Windows

```powershell
irm https://raw.githubusercontent.com/AncientiCe/mempalace-rs/main/scripts/install.ps1 | iex
```

The installer downloads the matching GitHub Release binary, verifies its SHA-256
checksum, installs it locally, and registers the MCP server with Cursor, Codex,
and Claude Code.

Development install:

```bash
cargo install --path .
mempalace install
```

The first time you run `mine`, the embedding model is downloaded automatically from HuggingFace and cached.

---

## Quick Start

```bash
cargo install --path .       # development install; release installers do this for you
mempalace install            # configures Cursor + Codex + Claude Code
mempalace doctor             # verifies MCP config, rules, binary, and drawer count
mempalace init ~/my-project  # detect rooms and write mempalace.yaml
mempalace mine ~/my-project  # populate the palace
```

Then restart your agent app so it reloads MCP configuration. Search manually with
`mempalace search "how did we decide on the database schema"` or let your agent
call the MCP tools when its installed rule tells it to consult memory.

---

## CLI Reference

| Command | Description |
|---|---|
| `mempalace init <dir>` | Detect rooms from folder structure, write `mempalace.yaml` |
| `mempalace mine <dir>` | Chunk, embed, and store project files |
| `mempalace mine-convos <dir>` | Ingest conversation exports |
| `mempalace search <query>` | Semantic search with similarity scores |
| `mempalace wake-up` | Print L0 (identity) + L1 (essential story) context |
| `mempalace status` | Palace overview: drawer counts by wing/room |
| `mempalace gain` | Show MCP usage gains, estimated savings, and per-project value |
| `mempalace split` | Split Claude Code mega-transcripts by session |
| `mempalace repair` | Re-embed any drawers missing vectors |
| `mempalace install` | Register the MCP server with Cursor, Codex, and Claude Code |
| `mempalace uninstall` | Remove MemPalace from MCP client configs |
| `mempalace doctor` | Inspect binary path, palace DB, and MCP config status |
| `mempalace mcp` | Start the MCP stdio server |

### `mine` flags

```bash
mempalace mine ~/my-project \
  --wing my_project          # Override wing name
  --limit 100                # Cap at 100 files
  --dry-run                  # Preview without storing
  --no-gitignore             # Ignore .gitignore rules
  --include vendor,third_party  # Force-include these paths
```

### `mine-convos` flags

```bash
mempalace mine-convos ~/Desktop/transcripts \
  --wing claude_sessions \
  --mode exchange   # or: general (decisions/milestones/emotions)
  --limit 50
  --dry-run
```

### `split` flags

```bash
mempalace split \
  --source ~/Desktop/transcripts \
  --min-sessions 2 \
  --dry-run
```

### `gain`

`mempalace gain` summarizes automatic MCP usage by Cursor, Codex, Claude Code,
or any other MCP client. It records local tool-call metadata in `palace.db` and
estimates value from retrieval hits, duplicate skips, KG facts, diary recalls,
repeat questions, and latency.

```bash
mempalace gain
mempalace gain --project my_project --since 7d
mempalace gain --history
mempalace gain --json
```

Example output:

```text
MemPalace gain - last 30d (mempalace_rs)
  Tool calls         : 412   (sessions: 27)
  Hit rate           : 88%   (search hits 142/162)
  Tokens saved (est) : ~78,400
  Re-index skipped   : 31    (duplicate drawers avoided)
  KG facts recalled  : 56
  Diary recalls      : 8
  Repeat Qs avoided  : 19
  p95 latency        : 41 ms
  Top wings          : mempalace_rs(120), checkout(40)
```

Set `MEMPALACE_GAIN_DISABLED=1` to disable usage recording.

---

## MCP Setup

`mempalace install` is the normal setup command for all three major agentic coding
clients. It writes both:

- an MCP server entry that starts `mempalace mcp`
- a small rule that tells the agent when to call `mempalace_status`,
  `mempalace_search`, `mempalace_kg_query`, and `mempalace_diary_write`

```bash
mempalace install
```

What gets written by default:

| Client | MCP config | Rule file |
|---|---|---|
| Cursor | `~/.cursor/mcp.json` | `~/.cursor/rules/mempalace.mdc` |
| Codex | `~/.codex/config.toml` | `~/.codex/AGENTS.md` |
| Claude Code | `~/.claude/mcp_servers.json` | `~/.claude/CLAUDE.md` |

Install for one client:

```bash
mempalace install --client cursor
mempalace install --client codex
mempalace install --client claude
```

Install project-scoped rules instead of global rules:

```bash
mempalace install --scope project --path /path/to/project
```

For project scope, Cursor also gets a project-local MCP config at
`<project>/.cursor/mcp.json`. Codex and Claude Code keep MCP config in their
user-level config files, while their rules go into `<project>/AGENTS.md` and
`<project>/CLAUDE.md`.

Skip rule files if you only want MCP wiring:

```bash
mempalace install --no-rule
```

Inspect the current setup:

```bash
mempalace doctor
```

Remove MemPalace config:

```bash
mempalace uninstall
mempalace uninstall --client cursor
```

### Cursor

After `mempalace install --client cursor`, restart Cursor or reload the window.
Settings -> MCP should show `mempalace` as an enabled stdio server.

Manual Cursor config shape:

```json
{
  "mcpServers": {
    "mempalace": {
      "command": "mempalace",
      "args": ["mcp"]
    }
  }
}
```

The rule is installed as `.cursor/rules/mempalace.mdc` with `alwaysApply: true`.

### Codex

After `mempalace install --client codex`, restart Codex so it reloads
`~/.codex/config.toml`.

Manual Codex config shape:

```toml
[mcp_servers.mempalace]
command = "mempalace"
args = ["mcp"]
```

The rule is installed as a managed MemPalace block in `~/.codex/AGENTS.md` (or
`<project>/AGENTS.md` with `--scope project`). Existing content is preserved.

### Claude Code

After `mempalace install --client claude`, restart Claude Code so it reloads
`~/.claude/mcp_servers.json`.

Manual Claude JSON shape is the same as Cursor's `mcpServers` object above.
You can also use Claude Code's own MCP command:

```bash
claude mcp remove mempalace
claude mcp add mempalace -- mempalace mcp
```

The rule is installed as a managed MemPalace block in `~/.claude/CLAUDE.md` (or
`<project>/CLAUDE.md` with `--scope project`). Existing content is preserved.

### MCP Tools

The server exposes tools for status, taxonomy, search, drawer CRUD, knowledge
graph operations, graph tunnels, hook acknowledgements, and agent diaries:

| Tool | Description |
|---|---|
| `mempalace_status` | Palace overview + protocol |
| `mempalace_gain` | MCP usage gains, estimated savings, and per-project value |
| `mempalace_list_wings` | List wings with drawer counts |
| `mempalace_list_rooms` | List rooms within a wing |
| `mempalace_get_taxonomy` | Full wing → room → count tree |
| `mempalace_get_aaak_spec` | AAAK compressed memory dialect spec |
| `mempalace_search` | Semantic search over drawers |
| `mempalace_check_duplicate` | Check if content already exists |
| `mempalace_add_drawer` | File content into the palace |
| `mempalace_delete_drawer` | Remove a drawer by ID |
| `mempalace_kg_query` | Query entity relationships |
| `mempalace_kg_add` | Add a fact (subject → predicate → object) |
| `mempalace_kg_invalidate` | Mark a fact as no longer true |
| `mempalace_kg_timeline` | Chronological fact history |
| `mempalace_kg_stats` | Knowledge graph overview |
| `mempalace_traverse` | BFS graph walk from a room |
| `mempalace_find_tunnels` | Rooms bridging two wings |
| `mempalace_graph_stats` | Palace graph summary |
| `mempalace_diary_write` | Write a diary entry in AAAK format |
| `mempalace_diary_read` | Read recent diary entries |

---

## Migration from Python

The Rust version uses a new single-file database (`palace.db`). Your existing ChromaDB data cannot be migrated automatically.

**Steps:**

```bash
# 1. Re-mine your projects
mempalace init ~/my-project && mempalace mine ~/my-project

# 2. Re-index conversations
mempalace mine-convos ~/Desktop/transcripts

# 3. Verify
mempalace status
```

Your `identity.txt`, `people_map.json`, and `known_names.json` in `~/.mempalace/` are compatible and will be read automatically.

---

## Test on a Project

```bash
mempalace init /path/to/project
mempalace mine /path/to/project
mempalace status
```

Restart Cursor, Codex, or Claude Code, then ask the agent a project question that
should use memory, for example: "Search MemPalace for how this project handles
database migrations." The agent should call `mempalace_search` through MCP
instead of re-indexing the repository from scratch.

---

## Configuration

`~/.mempalace/config.json` is read on startup. Environment variables take highest priority:

| Env Var | Default | Description |
|---|---|---|
| `MEMPALACE_PALACE_PATH` | `~/.mempalace/palace` | Palace data directory |

### `mempalace.yaml` (per-project)

Created by `mempalace init`. Example:

```yaml
wing: my_project
rooms:
  - name: backend
    description: Server and API code
    keywords: [api, server, routes, models]
  - name: frontend
    description: UI components
    keywords: [ui, components, pages, views]
  - name: general
    description: Everything else
    keywords: []
```

---

## Memory Stack

| Layer | Name | Description |
|---|---|---|
| L0 | Identity | `~/.mempalace/identity.txt` — always loaded (~100 tokens) |
| L1 | Essential Story | Top drawers by importance, grouped by room (~600–900 tokens) |
| L2 | On-Demand | Wing/room filtered retrieval |
| L3 | Deep Search | Full semantic search |

`mempalace wake-up` prints L0 + L1. The AI uses MCP tools for L2/L3.

---

## Development

```bash
cargo build
cargo test
cargo clippy
```

Tests use in-memory SQLite — no palace.db needed. The embedding model is not loaded in tests that don't require it.

---

## Hooks Compatibility

Shell hooks that previously called `python -m mempalace.mcp_server` can now call `mempalace mcp`. Update the binary path in your hooks:

```bash
# Before (Python)
exec python -m mempalace.mcp_server

# After (Rust)
exec mempalace mcp
```
