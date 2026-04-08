//! Basic MemPalace usage example.
//!
//! Run: cargo run --example basic_usage

use mempalace::db;
use mempalace::knowledge_graph;
use mempalace::store;

fn main() -> anyhow::Result<()> {
    // 1. Open an in-memory palace (use db::open(&config.palace_db_path()) for production)
    let conn = db::open_in_memory()?;

    println!("=== MemPalace Basic Usage ===\n");

    // 2. Add some drawers
    let (added, id) = store::add_drawer(
        &conn,
        "wing_code",
        "backend",
        "We use SQLite with WAL mode for the palace database because it supports concurrent reads.",
        None, // embedding (None = no vector, use embed_one() for real usage)
        "architecture.md",
        0,
        "example",
        4.0,
    )?;
    println!("Added drawer: {} (id: {})", added, &id[..20]);

    // 3. Add a knowledge graph fact
    let triple_id = knowledge_graph::add_triple(
        &conn,
        "MemPalace",
        "stores_data_in",
        "SQLite",
        Some("2026-04-01"),
        None,
        1.0,
        Some(&id),
        None,
    )?;
    println!("Added triple: {}", &triple_id[..20]);

    // 4. Query the KG
    let facts = knowledge_graph::query_entity(&conn, "MemPalace", None, "outgoing")?;
    println!("\nFacts about MemPalace:");
    for fact in &facts {
        println!("  {} → {} → {}", fact.subject, fact.predicate, fact.object);
    }

    // 5. Palace stats
    let total = store::count_drawers(&conn)?;
    let wings = store::wing_counts(&conn)?;
    println!("\nPalace stats:");
    println!("  Total drawers: {total}");
    println!("  Wings: {:?}", wings);

    let kg_stats = knowledge_graph::stats(&conn)?;
    println!("  KG entities: {}", kg_stats.entities);
    println!("  KG triples: {}", kg_stats.triples);

    println!("\nDone! For real usage, run: mempalace init <dir> && mempalace mine <dir>");
    Ok(())
}
