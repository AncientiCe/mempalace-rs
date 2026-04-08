# MemPalace RS

A local memory palace for AI assistants — complete Rust rewrite of [mempalace](../mempalace).

**Why Rust over Python for memory:**
- No GIL, no reference-counted leaks, deterministic `Drop`
- Embeddings + vector search happen natively in-process via ONNX Runtime ([fastembed](https://docs.rs/fastembed))
- Single statically-linked binary — install is `cargo install`
- Sync stdio MCP loop with zero overhead

## Storage

Collapses Python's dual-store (ChromaDB + SQLite) into **one file** at `~/.mempalace/palace.db`:

| Table | Purpose |
|---|---|
| `drawers` | Text content + embedding BLOB + metadata |
| `entities` | KG entity nodes |
| `triples` | KG temporal relationship edges |

Embeddings are 384-dim `f32` vectors from `all-MiniLM-L6-v2`. Search is brute-force cosine similarity — instant for < 500K drawers.

---

## Installation

```bash
git clone https://github.com/your-user/mempalace-rs
cd mempalace-rs
cargo install --path .
```

The first time you run `mine`, the embedding model (`all-MiniLM-L6-v2`, ~90 MB) is downloaded automatically from HuggingFace and cached.

---

## Quick Start

```bash
# 1. Detect rooms from your project folder structure
mempalace init ~/my-project

# 2. Index the project into the palace
mempalace mine ~/my-project

# 3. Index conversations
mempalace mine-convos ~/Desktop/transcripts

# 4. Search
mempalace search "how did we decide on the database schema"

# 5. Wake-up (L0 + L1 context for the AI)
mempalace wake-up
```

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
| `mempalace split` | Split Claude Code mega-transcripts by session |
| `mempalace repair` | Re-embed any drawers missing vectors |
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

---

## MCP Setup for Claude Code

Replace the Python server with the native Rust binary — zero config changes needed:

```bash
# Remove old Python MCP server
claude mcp remove mempalace

# Add Rust version (same tool names/schemas)
claude mcp add mempalace -- mempalace mcp
```

Or add manually to `~/.claude/mcp_servers.json`:

```json
{
  "mempalace": {
    "command": "mempalace",
    "args": ["mcp"]
  }
}
```

### 19 MCP Tools

All tool names and input schemas are identical to the Python version:

| Tool | Description |
|---|---|
| `mempalace_status` | Palace overview + protocol |
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

## 4-Layer Memory Stack

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
