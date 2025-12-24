// src/database.rs

use crate::models::JsonProblem;
use rusqlite::{params, Connection, Result};

pub fn init_db(conn: &Connection) -> Result<()> {
    println!("[DEBUG] init_db: Checking database schema...");

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS skills (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL
        );
        CREATE TABLE IF NOT EXISTS skill_prereqs (
            skill_id INTEGER,
            prereq_id INTEGER,
            PRIMARY KEY (skill_id, prereq_id)
        );
        CREATE TABLE IF NOT EXISTS problems (
            id INTEGER PRIMARY KEY,
            slug TEXT UNIQUE NOT NULL,
            title TEXT NOT NULL,
            url TEXT,
            difficulty TEXT CHECK (difficulty IN ('Easy','Medium','Hard'))
        );
        CREATE TABLE IF NOT EXISTS problem_skills (
            problem_id INTEGER,
            skill_id INTEGER,
            PRIMARY KEY (problem_id, skill_id)
        );
        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL
        );
        CREATE TABLE IF NOT EXISTS track_problems (
            track_id INTEGER,
            problem_id INTEGER,
            PRIMARY KEY (track_id, problem_id)
        );
        CREATE TABLE IF NOT EXISTS skill_state (
            skill_id INTEGER PRIMARY KEY,
            mastery REAL NOT NULL DEFAULT 0.0,
            attempts INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS attempts (
            id INTEGER PRIMARY KEY,
            problem_id INTEGER,
            time_minutes REAL,
            solved INTEGER,
            read_solution INTEGER,
            timestamp INTEGER
        );
        CREATE TABLE IF NOT EXISTS problem_state (
            problem_id INTEGER PRIMARY KEY,
            ease_factor REAL NOT NULL DEFAULT 2.5,
            interval_days REAL NOT NULL DEFAULT 1.0,
            next_review_ts INTEGER NOT NULL
        );
        ",
    )?;

    let count: i64 = conn.query_row("SELECT count(*) FROM problems", [], |row| row.get(0))?;
    if count == 0 {
        println!("[DEBUG] init_db: Table empty. Seeding data...");
        seed_data(conn)?;
    }

    Ok(())
}

fn seed_data(conn: &Connection) -> Result<()> {
    // 1. Skills
    let skills = vec![
        "Arrays and Hashing",
        "Two Pointers",
        "Stack",
        "Binary Search",
        "Sliding Window",
        "Linked List",
        "Tree",
        "Tries",
        "Heap / Priority Queue",
        "Backtracking",
        "Intervals",
        "Greedy",
        "Graph",
        "Graph2",
        "1DDP",
        "2DDP",
        "Bit Manipulation",
        "Math",
    ];

    let mut stmt = conn.prepare("INSERT OR IGNORE INTO skills (name) VALUES (?)")?;
    for s in &skills {
        stmt.execute([s])?;
    }

    // 2. Prereqs
    let dag = vec![
        ("Two Pointers", "Arrays and Hashing"),
        ("Stack", "Arrays and Hashing"),
        ("Binary Search", "Two Pointers"),
        ("Sliding Window", "Two Pointers"),
        ("Linked List", "Two Pointers"),
        ("Tree", "Binary Search"),
        ("Tree", "Linked List"),
        ("Tries", "Tree"),
        ("Heap / Priority Queue", "Tree"),
        ("Backtracking", "Tree"),
        ("Intervals", "Heap / Priority Queue"),
        ("Greedy", "Heap / Priority Queue"),
        ("Graph", "Backtracking"),
        ("Graph2", "Graph"),
        ("1DDP", "Backtracking"),
        ("2DDP", "1DDP"),
        ("2DDP", "Graph"),
        ("Bit Manipulation", "1DDP"),
        ("Math", "Bit Manipulation"),
        ("Math", "2DDP"),
    ];

    let mut stmt = conn.prepare("INSERT OR IGNORE INTO skill_prereqs (skill_id, prereq_id) SELECT s1.id, s2.id FROM skills s1, skills s2 WHERE s1.name = ? AND s2.name = ?")?;
    for (child, parent) in dag {
        stmt.execute(params![child, parent])?;
    }

    // 3. Tracks
    conn.execute(
        "INSERT OR IGNORE INTO tracks (name) VALUES ('NeetCode 150')",
        [],
    )?;

    // 4. Problems
    // Note: This relies on the file strictly existing in src/data/
    let data = include_str!("data/neetcode_150.json");
    let problems: Vec<JsonProblem> =
        serde_json::from_str(data).expect("Error parsing problems JSON");

    let mut p_stmt = conn.prepare(
        "INSERT OR REPLACE INTO problems (id, slug, title, difficulty, url) VALUES (?, ?, ?, ?, ?)",
    )?;
    let mut ps_stmt = conn.prepare("INSERT OR REPLACE INTO problem_skills (problem_id, skill_id) SELECT ?, id FROM skills WHERE name = ?")?;
    let mut tp_stmt =
        conn.prepare("INSERT OR REPLACE INTO track_problems (track_id, problem_id) VALUES (1, ?)")?;

    for p in problems {
        let slug = p.title.to_lowercase().replace(" ", "-");
        p_stmt.execute(params![p.id, slug, p.title, p.difficulty, p.url])?;
        ps_stmt.execute(params![p.id, p.category])?;
        tp_stmt.execute(params![p.id])?;
    }

    // 5. Init Skill State
    conn.execute(
        "INSERT OR IGNORE INTO skill_state (skill_id) SELECT id FROM skills",
        [],
    )?;

    Ok(())
}
