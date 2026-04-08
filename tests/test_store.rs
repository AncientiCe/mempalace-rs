use mempalace::db;
use mempalace::embedder;
use mempalace::store::*;

fn open_test_db() -> rusqlite::Connection {
    db::open_in_memory().expect("in-memory DB should open")
}

#[test]
fn add_and_get_drawer() {
    let conn = open_test_db();
    let (added, id) = add_drawer(&conn, "wing_code", "backend", "Hello world", None, "test.py", 0, "test", 3.0).unwrap();
    assert!(added, "should be added");
    assert!(id.starts_with("drawer_wing_code_backend_"));

    let drawer = get_drawer(&conn, &id).unwrap().expect("should find drawer");
    assert_eq!(drawer.wing, "wing_code");
    assert_eq!(drawer.room, "backend");
    assert_eq!(drawer.content, "Hello world");
}

#[test]
fn duplicate_add_returns_false() {
    let conn = open_test_db();
    let (added, _) = add_drawer(&conn, "wing_code", "backend", "Hello", None, "dup.py", 0, "test", 3.0).unwrap();
    assert!(added);
    let (added2, _) = add_drawer(&conn, "wing_code", "backend", "Hello", None, "dup.py", 0, "test", 3.0).unwrap();
    assert!(!added2, "duplicate should not be added");
}

#[test]
fn delete_drawer_works() {
    let conn = open_test_db();
    let (_, id) = add_drawer(&conn, "wing_a", "room_a", "delete me", None, "x.txt", 0, "test", 3.0).unwrap();
    let deleted = delete_drawer(&conn, &id).unwrap();
    assert!(deleted);
    assert!(get_drawer(&conn, &id).unwrap().is_none());
}

#[test]
fn wing_counts_aggregation() {
    let conn = open_test_db();
    add_drawer(&conn, "wing_a", "r1", "a1", None, "a1.txt", 0, "test", 3.0).unwrap();
    add_drawer(&conn, "wing_a", "r2", "a2", None, "a2.txt", 0, "test", 3.0).unwrap();
    add_drawer(&conn, "wing_b", "r1", "b1", None, "b1.txt", 0, "test", 3.0).unwrap();

    let wings = wing_counts(&conn).unwrap();
    assert_eq!(wings["wing_a"], 2);
    assert_eq!(wings["wing_b"], 1);
}

#[test]
fn file_already_mined_check() {
    let conn = open_test_db();
    assert!(!file_already_mined(&conn, "unique_file.txt").unwrap());
    add_drawer(&conn, "wing_x", "room_x", "content", None, "unique_file.txt", 0, "test", 3.0).unwrap();
    assert!(file_already_mined(&conn, "unique_file.txt").unwrap());
}

#[test]
fn vector_search_returns_results() {
    let conn = open_test_db();

    // Use random-ish f32 vectors (same dimension as real embeddings)
    let v1: Vec<f32> = (0..384).map(|i| (i as f32).sin()).collect();
    let v2: Vec<f32> = (0..384).map(|i| (i as f32).cos()).collect();
    let v3: Vec<f32> = (0..384).map(|i| -(i as f32).sin()).collect();

    add_drawer(&conn, "w", "r", "first doc", Some(&v1), "a.txt", 0, "test", 3.0).unwrap();
    add_drawer(&conn, "w", "r", "second doc", Some(&v2), "b.txt", 0, "test", 3.0).unwrap();
    add_drawer(&conn, "w", "r", "opposite doc", Some(&v3), "c.txt", 0, "test", 3.0).unwrap();

    // Search with v1 — "first doc" should rank highest
    let filter = DrawerFilter::default();
    let results = vector_search(&conn, &v1, &filter, 3).unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].text, "first doc", "first doc should be best match");
}

#[test]
fn taxonomy_works() {
    let conn = open_test_db();
    add_drawer(&conn, "wing_code", "backend", "c1", None, "f1", 0, "test", 3.0).unwrap();
    add_drawer(&conn, "wing_code", "frontend", "c2", None, "f2", 0, "test", 3.0).unwrap();
    add_drawer(&conn, "wing_docs", "readme", "c3", None, "f3", 0, "test", 3.0).unwrap();

    let tax = taxonomy(&conn).unwrap();
    assert_eq!(tax["wing_code"]["backend"], 1);
    assert_eq!(tax["wing_code"]["frontend"], 1);
    assert_eq!(tax["wing_docs"]["readme"], 1);
}
