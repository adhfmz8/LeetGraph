// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::prelude::*;
use rusqlite::OptionalExtension;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use tauri::Manager;
use tauri::State;

// --- Constants ---
const ALPHA: f64 = 0.15; // Mastery gain per solve
const INTERVAL_MIN: f64 = 1.0; // Minimum interval (days)
const INTERVAL_MAX: f64 = 180.0; // Max interval (6 months)
const DAY_SECONDS: i64 = 86400; // Seconds in a day

// --- Data Structures ---

#[derive(Serialize, Deserialize, Debug)]
struct ProblemView {
    id: i64,
    title: String,
    url: String,
    difficulty: String,
    track_name: String,
}

#[derive(Deserialize)]
struct AttemptLog {
    problem_id: i64,
    time_minutes: f64,
    solved: bool,
    read_solution: bool,
}

#[derive(Deserialize)]
struct JsonProblem {
    id: i64,
    title: String,
    difficulty: String,
    category: String,
    url: String,
}

struct AppState {
    db: Mutex<Connection>,
}

// --- Database Init & Seeding ---

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    webbrowser::open(&url).map_err(|e| e.to_string())
}

fn init_db(conn: &Connection) -> Result<()> {
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

    // Check if seeded
    let count: i64 = conn.query_row("SELECT count(*) FROM problems", [], |row| row.get(0))?;
    if count == 0 {
        println!("[DEBUG] init_db: Table empty. Seeding data...");
        seed_data(conn)?;
    }

    Ok(())
}

fn seed_data(conn: &Connection) -> Result<()> {
    // A. Define Categories (Skills)
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

    // B. Build Dependency Graph (Prereqs)
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

    // C. Insert Tracks
    conn.execute(
        "INSERT OR IGNORE INTO tracks (name) VALUES ('NeetCode 150')",
        [],
    )?;

    // D. Insert Problems from JSON
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

    // E. Initialize Skill State
    conn.execute(
        "INSERT OR IGNORE INTO skill_state (skill_id) SELECT id FROM skills",
        [],
    )?;

    Ok(())
}

// --- Core Algorithm Logic ---

fn current_time() -> i64 {
    Utc::now().timestamp()
}

// Check if a skill is unlocked based on prereqs
fn is_skill_unlocked(conn: &Connection, skill_id: i64) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT count(*) FROM skill_prereqs sp
         JOIN skill_state ss ON sp.prereq_id = ss.skill_id
         WHERE sp.skill_id = ?
         AND (ss.mastery < 0.7 OR (ss.mastery < 0.9 AND ss.attempts < 2))",
    )?;
    let failed_prereqs: i64 = stmt.query_row([skill_id], |row| row.get(0))?;
    Ok(failed_prereqs == 0)
}

// Fallback logic if everything is reviewed/locked
fn pick_cram_problem(conn: &Connection, track_id: i64) -> Result<Option<i64>> {
    let mut stmt = conn.prepare(
        "SELECT p.id
         FROM problems p
         JOIN track_problems tp ON p.id = tp.problem_id
         JOIN problem_skills ps ON p.id = ps.problem_id
         JOIN skill_state ss ON ps.skill_id = ss.skill_id
         WHERE tp.track_id = ?
         ORDER BY ss.mastery ASC
         LIMIT 1",
    )?;

    let pid = stmt.query_row([track_id], |row| row.get(0));
    match pid {
        Ok(id) => Ok(Some(id)),
        Err(_) => Ok(None),
    }
}

// --- Tauri Commands ---

#[tauri::command]
fn get_next_problem(state: State<AppState>) -> Result<Option<ProblemView>, String> {
    println!("\n[DEBUG] --- CMD: get_next_problem ---");
    let conn = state.db.lock().unwrap();
    let now = current_time();
    let track_id = 1;

    // --- PRIORITY 1: REVIEWS (Memory Protection) ---
    // Unchanged: We must review what we are about to forget.
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.title, p.difficulty, p.url 
         FROM problem_state ps
         JOIN problems p ON ps.problem_id = p.id
         WHERE ps.next_review_ts <= ?
         ORDER BY ps.next_review_ts ASC
         LIMIT 1",
        )
        .map_err(|e| e.to_string())?;

    let review_prob = stmt
        .query_row([now], |row| {
            Ok(ProblemView {
                id: row.get(0)?,
                title: row.get(1)?,
                difficulty: row.get(2)?,
                url: row.get(3)?,
                track_name: "NeetCode 150 (Review)".to_string(),
            })
        })
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(p) = review_prob {
        println!("[DEBUG] Found due review: {} (ID: {})", p.title, p.id);
        return Ok(Some(p));
    }

    // --- PRIORITY 2: DISCOVERY (Skill Expansion) ---
    // Logic Update: Prioritize Easy problems, then Randomize.
    let mut stmt = conn
        .prepare("SELECT id FROM skills")
        .map_err(|e| e.to_string())?;
    let skills_iter = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;

    let mut unlocked_skills = Vec::new();
    for s in skills_iter {
        let sid = s.map_err(|e| e.to_string())?;
        if is_skill_unlocked(&conn, sid).unwrap_or(false) {
            unlocked_skills.push(sid);
        }
    }

    if unlocked_skills.is_empty() {
        return Ok(None);
    }

    let placeholders = unlocked_skills
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    // UPDATED SQL:
    // 1. Filter by Track and Unlocked Skills
    // 2. Exclude problems already in 'problem_state' (seen problems)
    // 3. Sort by Difficulty (Easy=1, Medium=2, Hard=3)
    // 4. Randomize within the same difficulty tier
    let sql = format!(
        "SELECT p.id, p.title, p.difficulty, p.url 
         FROM problems p
         JOIN track_problems tp ON p.id = tp.problem_id
         JOIN problem_skills ps ON p.id = ps.problem_id
         WHERE tp.track_id = ?
         AND ps.skill_id IN ({})
         AND p.id NOT IN (SELECT problem_id FROM problem_state)
         GROUP BY p.id
         ORDER BY 
            CASE p.difficulty 
                WHEN 'Easy' THEN 1 
                WHEN 'Medium' THEN 2 
                WHEN 'Hard' THEN 3 
                ELSE 4 
            END ASC, 
            RANDOM()
         LIMIT 1",
        placeholders
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    params.push(Box::new(track_id));
    for u in &unlocked_skills {
        params.push(Box::new(*u));
    }

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let new_prob = stmt
        .query_row(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(ProblemView {
                id: row.get(0)?,
                title: row.get(1)?,
                difficulty: row.get(2)?,
                url: row.get(3)?,
                track_name: "NeetCode 150 (New)".to_string(),
            })
        })
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(p) = new_prob {
        println!("[DEBUG] Found new problem: {} (ID: {})", p.title, p.id);
        return Ok(Some(p));
    }

    // --- PRIORITY 3: CRAM/GRIND ---
    println!("[DEBUG] No reviews or new problems. Entering Cram mode.");
    pick_cram_problem(&conn, track_id)
        .map(|opt| {
            opt.map(|id| {
                let mut s = conn
                    .prepare("SELECT title, difficulty, url FROM problems WHERE id = ?")
                    .unwrap();
                s.query_row([id], |row| {
                    Ok(ProblemView {
                        id,
                        title: row.get(0).unwrap(),
                        difficulty: row.get(1).unwrap(),
                        url: row.get(2).unwrap(),
                        track_name: "Cram Mode".to_string(),
                    })
                })
                .unwrap()
            })
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn submit_attempt(state: State<AppState>, log: AttemptLog) -> Result<(), String> {
    println!("\n[DEBUG] --- CMD: submit_attempt (Pedagogy V2) ---");
    let conn = state.db.lock().unwrap();
    let now = current_time();

    // 1. Log History
    conn.execute(
        "INSERT INTO attempts (problem_id, time_minutes, solved, read_solution, timestamp) VALUES (?, ?, ?, ?, ?)",
        params![log.problem_id, log.time_minutes, log.solved, log.read_solution, now]
    ).map_err(|e| e.to_string())?;

    // 2. Gather Context (Difficulty & History)
    let diff_str: String = conn
        .query_row(
            "SELECT difficulty FROM problems WHERE id = ?",
            [log.problem_id],
            |r| r.get(0),
        )
        .unwrap_or("Medium".to_string());

    // Check how many times we've tried this before (excluding this specific just-inserted log if strict,
    // but usually count(*) includes the one we just added. Let's subtract 1 or check distinct timestamps.
    // Simpler: Just check if there is MORE than 1 attempt now.)
    let attempt_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM attempts WHERE problem_id = ?",
            [log.problem_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let is_new_problem = attempt_count <= 1;

    // Get previous state
    let mut stmt = conn
        .prepare("SELECT ease_factor, interval_days FROM problem_state WHERE problem_id = ?")
        .map_err(|e| e.to_string())?;

    let (mut ease, prev_interval) = stmt
        .query_row([log.problem_id], |row| {
            Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?))
        })
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or((2.5, 0.0)); // Default state

    // 3. Define Expectations
    let expected_mins = match diff_str.as_str() {
        "Easy" => 10.0,
        "Medium" => 25.0,
        _ => 45.0,
    };
    let time_ratio = log.time_minutes / expected_mins;

    // 4. Algorithm: "The Grit vs. Recall Split"
    let mut interval = prev_interval;
    let is_fail = !log.solved || log.read_solution;

    // We calculate a 'grade' for internal logic (0=Fail, 1=Hard, 2=Good, 3=Easy)
    // But for SM-2 we update ease directly.

    if is_fail {
        // --- FAILURE ---
        ease = (ease - 0.20).max(1.3);
        interval = INTERVAL_MIN; // Reset to Day 1
        println!("[Logic] Fail. Resetting.");
    } else if is_new_problem {
        // --- NEW PROBLEM (Learning Phase) ---
        // We do NOT punish slowness here. If they solved it, they learn.

        if time_ratio > 1.5 {
            // "Grit Bonus": They struggled but won.
            // Keep ease neutral (don't increase it, don't decrease it much).
            // They need to see it again soon to solidify the complex pattern.
            ease -= 0.05;
            interval = 2.0; // Short interval to consolidate
            println!("[Logic] Grit Solve. Short interval to consolidate.");
        } else {
            // "Clean Solve": Standard expansion
            ease += 0.15;
            interval = 4.0; // Push it out a bit further
            println!("[Logic] Clean First Solve.");
        }
    } else {
        // --- REVIEW PROBLEM (Memory Phase) ---
        // Here, slowness IS a penalty because it implies the pattern wasn't instant.

        if time_ratio > 2.0 {
            // "Struggle Review": Forgot the trick.
            ease -= 0.15;
            interval = interval * 0.7; // Shrink interval (re-learning)
            println!("[Logic] Slow Review. Shrinking interval.");
        } else if time_ratio < 0.6 {
            // "Speed Review": Muscle memory.
            ease += 0.15;
            interval = interval * ease * 1.2; // Bonus expansion
            println!("[Logic] Fast Review. Bonus expansion.");
        } else {
            // "Standard Review"
            interval = interval * ease;
        }
    }

    // Clamp and Save
    ease = ease.clamp(1.3, 5.0);
    interval = interval.clamp(INTERVAL_MIN, INTERVAL_MAX);
    let next_ts = now + ((interval * DAY_SECONDS as f64) as i64);

    conn.execute(
        "INSERT OR REPLACE INTO problem_state (problem_id, ease_factor, interval_days, next_review_ts) VALUES (?, ?, ?, ?)",
        params![log.problem_id, ease, interval, next_ts]
    ).map_err(|e| e.to_string())?;

    // 5. Update Mastery (The "Gatekeeper" Logic)

    // Config: How much XP does a problem give?
    let difficulty_multiplier = match diff_str.as_str() {
        "Easy" => 0.8,   // Easy problems give less mastery
        "Medium" => 1.2, // Standard
        "Hard" => 1.5,   // Hard problems unlock trees faster
        _ => 1.0,
    };

    // Config: How was the performance?
    let performance_multiplier = if is_fail {
        0.0
    } else if is_new_problem {
        if time_ratio > 1.5 {
            1.2
        } else {
            1.0
        } // 20% Bonus XP for "Grit" (sticking with it)
    } else {
        0.3 // Reviews give very little mastery (maintenance only)
    };

    let final_delta = ALPHA * difficulty_multiplier * performance_multiplier;

    // Apply to all associated skills
    let mut stmt = conn
        .prepare("SELECT skill_id FROM problem_skills WHERE problem_id = ?")
        .map_err(|e| e.to_string())?;
    let skills: Vec<i64> = stmt
        .query_map([log.problem_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .map(|r| r.unwrap())
        .collect();

    for sid in skills {
        let mut s_stmt = conn
            .prepare("SELECT mastery, attempts FROM skill_state WHERE skill_id = ?")
            .map_err(|e| e.to_string())?;
        let (mut mastery, attempts): (f64, i32) = s_stmt
            .query_row([sid], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap_or((0.0, 0));

        mastery = (mastery + final_delta).clamp(0.0, 1.0);

        conn.execute(
            "UPDATE skill_state SET mastery = ?, attempts = ? WHERE skill_id = ?",
            params![mastery, attempts + 1, sid],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // 1. Resolve the Application Support directory
            // Path becomes: ~/Library/Application Support/com.your-identifier.app/neetcode_trainer.db
            let app_handle = app.handle();
            let app_data_dir = app_handle
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");

            // 2. Create the directory if it doesn't exist
            if !app_data_dir.exists() {
                fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            }

            // 3. Construct the full path
            let db_path = app_data_dir.join("neetcode_trainer.db");
            println!("Database path: {:?}", db_path); // For debugging

            // 4. Initialize Connection
            let conn = Connection::open(db_path).expect("Failed to open DB");
            init_db(&conn).expect("Failed to init DB");

            // 5. Manage State
            app.manage(AppState {
                db: Mutex::new(conn),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_next_problem,
            submit_attempt,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
