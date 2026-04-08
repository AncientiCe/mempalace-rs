use mempalace::miner::{chunk_text, detect_room, CHUNK_SIZE};
use mempalace::room_detector::Room;
use std::path::Path;

#[test]
fn chunk_text_short_content_produces_one_chunk() {
    // Text must be >= MIN_CHUNK_SIZE (50 chars) to produce a chunk
    let text = "Hello world, this is a somewhat longer sentence that exceeds minimum size.";
    let chunks = chunk_text(text);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].1, 0);
}

#[test]
fn chunk_text_long_content_produces_multiple_chunks() {
    let text = "a".repeat(CHUNK_SIZE * 3);
    let chunks = chunk_text(&text);
    assert!(
        chunks.len() >= 2,
        "long content should produce multiple chunks"
    );
}

#[test]
fn chunk_text_respects_paragraph_breaks() {
    let text = format!("{}\n\n{}", "a".repeat(600), "b".repeat(600));
    let chunks = chunk_text(&text);
    // Should split at paragraph boundary
    assert!(chunks.len() >= 2, "should respect paragraph breaks");
}

#[test]
fn chunk_text_indices_are_sequential() {
    let text = "w".repeat(CHUNK_SIZE * 4);
    let chunks = chunk_text(&text);
    for (i, (_, idx)) in chunks.iter().enumerate() {
        assert_eq!(*idx, i, "chunk indices should be sequential");
    }
}

#[test]
fn detect_room_uses_folder_path() {
    let rooms = vec![
        Room {
            name: "backend".into(),
            description: "backend code".into(),
            keywords: vec!["api".into()],
        },
        Room {
            name: "frontend".into(),
            description: "ui code".into(),
            keywords: vec!["ui".into()],
        },
        Room {
            name: "general".into(),
            description: "other".into(),
            keywords: vec![],
        },
    ];
    let project = Path::new("/project");
    let path = Path::new("/project/backend/server.py");
    let room = detect_room(path, "some content", &rooms, project);
    assert_eq!(room, "backend");
}

#[test]
fn detect_room_keyword_scoring() {
    let rooms = vec![
        Room {
            name: "database".into(),
            description: "db".into(),
            keywords: vec!["sql".into(), "schema".into()],
        },
        Room {
            name: "general".into(),
            description: "other".into(),
            keywords: vec![],
        },
    ];
    let project = Path::new("/project");
    let path = Path::new("/project/config.txt");
    let content = "CREATE TABLE users (id INT); ALTER TABLE schema.sql;";
    let room = detect_room(path, content, &rooms, project);
    assert_eq!(room, "database");
}

#[test]
fn detect_room_defaults_to_general() {
    let rooms = vec![
        Room {
            name: "backend".into(),
            description: "backend".into(),
            keywords: vec!["rust".into()],
        },
        Room {
            name: "general".into(),
            description: "other".into(),
            keywords: vec![],
        },
    ];
    let project = Path::new("/project");
    let path = Path::new("/project/data.csv");
    let room = detect_room(path, "some random content xyz", &rooms, project);
    assert_eq!(room, "general");
}
