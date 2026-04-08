//! Project file ingestor.
//!
//! Reads mempalace.yaml, walks project with gitignore respect (via `ignore` crate),
//! chunks text (~800 chars, 100 overlap), routes to rooms, embeds and stores drawers.
//! Port of miner.py.

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

use crate::room_detector::{load_config, Room};
use crate::store::{add_drawer, file_already_mined};

pub const CHUNK_SIZE: usize = 800;
pub const CHUNK_OVERLAP: usize = 100;
pub const MIN_CHUNK_SIZE: usize = 50;

static READABLE_EXTENSIONS: &[&str] = &[
    "txt", "md", "py", "js", "ts", "jsx", "tsx", "json", "yaml", "yml",
    "html", "css", "java", "go", "rs", "rb", "sh", "csv", "sql", "toml",
];

static SKIP_FILENAMES: &[&str] = &[
    "mempalace.yaml",
    "mempalace.yml",
    "mempal.yaml",
    "mempal.yml",
    ".gitignore",
    "package-lock.json",
];

/// Split content into overlapping chunks, preferring paragraph/line boundaries.
pub fn chunk_text(content: &str) -> Vec<(String, usize)> {
    let content = content.trim();
    if content.is_empty() {
        return vec![];
    }

    let mut chunks = Vec::new();
    let bytes = content.as_bytes();
    let total = bytes.len();
    let mut start = 0;
    let mut chunk_index = 0;

    while start < total {
        let end = (start + CHUNK_SIZE).min(total);
        let mut cut = end;

        // Try to break at double newline first
        if cut < total {
            if let Some(pos) = content[start..cut].rfind("\n\n") {
                let abs = start + pos;
                if abs > start + CHUNK_SIZE / 2 {
                    cut = abs;
                }
            } else if let Some(pos) = content[start..cut].rfind('\n') {
                let abs = start + pos;
                if abs > start + CHUNK_SIZE / 2 {
                    cut = abs;
                }
            }
        }

        let chunk = content[start..cut].trim().to_string();
        if chunk.len() >= MIN_CHUNK_SIZE {
            chunks.push((chunk, chunk_index));
            chunk_index += 1;
        }

        if cut >= total {
            break;
        }
        start = cut.saturating_sub(CHUNK_OVERLAP);
    }

    chunks
}

/// Route a file to the correct room based on path, filename, and keyword scoring.
pub fn detect_room(
    filepath: &Path,
    content: &str,
    rooms: &[Room],
    project_path: &Path,
) -> String {
    let relative = filepath
        .strip_prefix(project_path)
        .unwrap_or(filepath)
        .to_string_lossy()
        .to_lowercase();
    let filename = filepath
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let content_lower = content.get(..2000.min(content.len())).unwrap_or(content).to_lowercase();

    // Priority 1: folder path matches room name or keywords
    let path_parts: Vec<&str> = relative.split('/').collect();
    for part in path_parts.iter().take(path_parts.len().saturating_sub(1)) {
        for room in rooms {
            let candidates: Vec<String> = std::iter::once(room.name.to_lowercase())
                .chain(room.keywords.iter().map(|k| k.to_lowercase()))
                .collect();
            if candidates.iter().any(|c| part == c || c.contains(part) || part.contains(c.as_str())) {
                return room.name.clone();
            }
        }
    }

    // Priority 2: filename matches room name
    for room in rooms {
        if room.name.to_lowercase().contains(&filename) || filename.contains(&room.name.to_lowercase()) {
            return room.name.clone();
        }
    }

    // Priority 3: keyword scoring
    let mut scores: HashMap<&str, usize> = HashMap::new();
    for room in rooms {
        let keywords: Vec<String> = std::iter::once(room.name.clone())
            .chain(room.keywords.iter().cloned())
            .collect();
        let score: usize = keywords.iter().map(|kw| {
            let kw_lower = kw.to_lowercase();
            content_lower.matches(kw_lower.as_str()).count()
        }).sum();
        if score > 0 {
            scores.insert(&room.name, score);
        }
    }

    if let Some(best) = scores.iter().max_by_key(|(_, v)| **v) {
        return best.0.to_string();
    }

    "general".to_string()
}

/// Mine a project directory into the palace.
pub fn mine(
    conn: &mut Connection,
    project_dir: &Path,
    wing_override: Option<&str>,
    agent: &str,
    limit: usize,
    dry_run: bool,
    respect_gitignore: bool,
    include_ignored: &[String],
) -> Result<()> {
    let project_path = project_dir.canonicalize().context("resolving project dir")?;
    let config = load_config(&project_path)?;
    let wing = wing_override.unwrap_or(&config.wing).to_string();
    let rooms = config.rooms;

    // Collect files using `ignore` crate (gitignore-aware)
    let mut walker = WalkBuilder::new(&project_path);
    walker
        .hidden(false)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore);

    // Force-include paths that are normally ignored
    for path in include_ignored {
        walker.add(project_path.join(path));
    }

    let mut files: Vec<std::path::PathBuf> = walker
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        .map(|e| e.path().to_path_buf())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            if SKIP_FILENAMES.contains(&name.as_ref()) {
                return false;
            }
            let ext = p.extension().unwrap_or_default().to_string_lossy().to_lowercase();
            READABLE_EXTENSIONS.contains(&ext.as_str())
        })
        .collect();

    if limit > 0 {
        files.truncate(limit);
    }

    println!("\n{}", "=".repeat(55));
    println!("  MemPalace Mine");
    println!("{}", "=".repeat(55));
    println!("  Wing:    {wing}");
    println!(
        "  Rooms:   {}",
        rooms.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
    );
    println!("  Files:   {}", files.len());
    if dry_run {
        println!("  DRY RUN — nothing will be filed");
    }
    println!("{}\n", "-".repeat(55));

    let mut total_drawers = 0usize;
    let mut files_skipped = 0usize;
    let mut room_counts: HashMap<String, usize> = HashMap::new();

    for (i, filepath) in files.iter().enumerate() {
        let source_file = filepath.to_string_lossy().to_string();

        if !dry_run && file_already_mined(conn, &source_file)? {
            files_skipped += 1;
            continue;
        }

        let content = match std::fs::read_to_string(filepath) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let content = content.trim().to_string();
        if content.len() < MIN_CHUNK_SIZE {
            continue;
        }

        let room = detect_room(filepath, &content, &rooms, &project_path);
        let chunks = chunk_text(&content);

        if dry_run {
            println!(
                "    [DRY RUN] {} → room:{room} ({} drawers)",
                filepath.file_name().unwrap_or_default().to_string_lossy(),
                chunks.len()
            );
            total_drawers += chunks.len();
            *room_counts.entry(room).or_default() += 1;
            continue;
        }

        let mut drawers_added = 0usize;
        for (chunk_text, chunk_index) in &chunks {
            let embedding = crate::embedder::embed_one(chunk_text).ok();
            let (added, _) = add_drawer(
                conn,
                &wing,
                &room,
                chunk_text,
                embedding.as_deref(),
                &source_file,
                *chunk_index,
                agent,
                3.0,
            )?;
            if added {
                drawers_added += 1;
            }
        }

        if drawers_added > 0 {
            *room_counts.entry(room.clone()).or_default() += 1;
            total_drawers += drawers_added;
            println!(
                "  ✓ [{:4}/{}] {:50} +{drawers_added}",
                i + 1,
                files.len(),
                filepath.file_name().unwrap_or_default().to_string_lossy()
            );
        }
    }

    println!("\n{}", "=".repeat(55));
    println!("  Done.");
    println!("  Files processed: {}", files.len() - files_skipped);
    println!("  Files skipped (already filed): {files_skipped}");
    println!("  Drawers filed: {total_drawers}");
    println!("\n  By room:");
    let mut sorted: Vec<(&String, &usize)> = room_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (room, count) in sorted {
        println!("    {:20} {count} files", room);
    }
    println!();
    println!("{}", "=".repeat(55));

    Ok(())
}

/// Re-embed all drawers that are missing embeddings.
pub fn repair(conn: &mut Connection) -> Result<()> {
    let unembedded = crate::store::fetch_unembedded(conn)?;
    println!("  Repairing {} drawers missing embeddings...", unembedded.len());

    for (id, content) in &unembedded {
        if let Ok(vec) = crate::embedder::embed_one(content) {
            crate::store::update_embedding(conn, id, &vec)?;
        }
    }
    println!("  Repair complete.");
    Ok(())
}

/// Print palace status.
pub fn status(conn: &Connection, palace_path: &Path) -> Result<()> {
    let total = crate::store::count_drawers(conn)?;
    let wings = crate::store::wing_counts(conn)?;

    println!("\n{}", "=".repeat(55));
    println!("  MemPalace Status — {total} drawers");
    println!("  Palace: {}", palace_path.display());
    println!("{}\n", "=".repeat(55));

    let mut sorted_wings: Vec<(&String, &i64)> = wings.iter().collect();
    sorted_wings.sort_by_key(|(k, _)| k.as_str());

    for (wing, _) in &sorted_wings {
        println!("  WING: {wing}");
        let rooms = crate::store::room_counts(conn, Some(wing))?;
        let mut sorted_rooms: Vec<(&String, &i64)> = rooms.iter().collect();
        sorted_rooms.sort_by(|a, b| b.1.cmp(a.1));
        for (room, count) in sorted_rooms {
            println!("    ROOM: {:20} {:5} drawers", room, count);
        }
        println!();
    }
    println!("{}\n", "=".repeat(55));
    Ok(())
}
