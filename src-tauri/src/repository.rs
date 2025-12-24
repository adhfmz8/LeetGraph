// src/repository.rs

use crate::constants::*;
use crate::models::{Difficulty, ProblemRepetitionState, ProblemView, SkillMasteryState};
use log::debug;
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::str::FromStr;

/// Fetches the current mastery state for a specific skill.
pub fn get_skill_state(conn: &Connection, skill_id: i64) -> Result<SkillMasteryState> {
    conn.query_row(
        "SELECT mastery, attempts FROM skill_state WHERE skill_id = ?",
        [skill_id],
        |row| {
            Ok(SkillMasteryState {
                skill_id,
                mastery: row.get(0)?,
                attempts: row.get(1)?,
            })
        },
    )
    .or_else(|_| {
        Ok(SkillMasteryState {
            skill_id,
            mastery: 0.0,
            attempts: 0,
        })
    })
}

/// Checks if a problem ID is an alternative.
/// Returns (parent_id, is_alternative).
/// If it's a normal problem, returns (id, false).
pub fn resolve_parent_id(conn: &Connection, problem_id: i64) -> Result<(i64, bool)> {
    let parent_id: Option<i64> = conn
        .query_row(
            "SELECT parent_id FROM alternatives WHERE id = ?",
            [problem_id],
            |row| row.get(0),
        )
        .optional()?;

    match parent_id {
        Some(pid) => Ok((pid, true)),
        None => Ok((problem_id, false)),
    }
}

/// Tries to find a random alternative for a given parent problem ID.
pub fn get_random_alternative(conn: &Connection, parent_id: i64) -> Result<Option<ProblemView>> {
    let result = conn
        .query_row(
            "SELECT id, title, difficulty, url 
         FROM alternatives 
         WHERE parent_id = ? 
         ORDER BY RANDOM() 
         LIMIT 1",
            [parent_id],
            |row| {
                Ok(ProblemView {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    difficulty: row.get(2)?,
                    url: row.get(3)?,
                    track_name: "🔀 Concept Variation".to_string(),
                    skills: Vec::new(),
                })
            },
        )
        .optional()?;

    if let Some(mut p) = result {
        // Alternatives might not have direct skill mappings in your DB design,
        // fallback to parent skills if needed, or if alternatives share skills:
        p.skills = get_skill_names_for_problem(conn, parent_id).unwrap_or_default();
        return Ok(Some(p));
    }
    Ok(None)
}

/// Updates the mastery state for a skill.
pub fn update_skill_state(conn: &Connection, state: &SkillMasteryState) -> Result<()> {
    conn.execute(
        "UPDATE skill_state SET mastery = ?, attempts = ? WHERE skill_id = ?",
        params![state.mastery, state.attempts, state.skill_id],
    )?;
    Ok(())
}

/// Fetches repetition state (SM-2) for a problem.
pub fn get_problem_repetition_state(
    conn: &Connection,
    problem_id: i64,
) -> Result<ProblemRepetitionState> {
    conn.query_row(
        "SELECT ease_factor, interval_days FROM problem_state WHERE problem_id = ?",
        [problem_id],
        |row| {
            Ok(ProblemRepetitionState {
                problem_id,
                ease_factor: row.get(0)?,
                interval_days: row.get(1)?,
                next_review_ts: 0, // Not needed for logic calc, overwritten on save
            })
        },
    )
    .optional()?
    .map_or(
        Ok(ProblemRepetitionState {
            problem_id,
            ease_factor: EASE_FACTOR_DEFAULT,
            interval_days: 0.0,
            next_review_ts: 0,
        }),
        Ok,
    )
}

/// Saves the calculated repetition state.
pub fn save_problem_repetition_state(
    conn: &Connection,
    state: &ProblemRepetitionState,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO problem_state (problem_id, ease_factor, interval_days, next_review_ts) VALUES (?, ?, ?, ?)",
        params![state.problem_id, state.ease_factor, state.interval_days, state.next_review_ts]
    )?;
    Ok(())
}

pub fn get_skill_names_for_problem(conn: &Connection, problem_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT s.name 
         FROM skills s
         JOIN problem_skills ps ON s.id = ps.skill_id
         WHERE ps.problem_id = ?",
    )?;

    let skills = stmt
        .query_map([problem_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    Ok(skills)
}

/// Records a raw attempt log.
pub fn log_attempt(
    conn: &Connection,
    problem_id: i64,
    time_minutes: f64,
    solved: bool,
    read_solution: bool,
    timestamp: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO attempts (problem_id, time_minutes, solved, read_solution, timestamp) VALUES (?, ?, ?, ?, ?)",
        params![problem_id, time_minutes, solved, read_solution, timestamp]
    )?;
    Ok(())
}

/// Helper to get difficulty and associated skills for a problem.
pub fn get_problem_metadata(conn: &Connection, problem_id: i64) -> Result<(Difficulty, Vec<i64>)> {
    let diff_str: String = conn.query_row(
        "SELECT difficulty FROM problems WHERE id = ?",
        [problem_id],
        |r| r.get(0),
    )?;

    let difficulty = Difficulty::from_str(&diff_str).unwrap_or(Difficulty::Medium);

    let mut stmt = conn.prepare("SELECT skill_id FROM problem_skills WHERE problem_id = ?")?;
    let skills = stmt
        .query_map([problem_id], |row| row.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;

    Ok((difficulty, skills))
}

pub fn get_attempt_count(conn: &Connection, problem_id: i64) -> Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM attempts WHERE problem_id = ?",
        [problem_id],
        |r| r.get(0),
    )
}

// --- Queries for "Get Next Problem" ---

pub fn find_due_review(conn: &Connection, now_ts: i64) -> Result<Option<ProblemView>> {
    let result = conn
        .query_row(
            "SELECT p.id, p.title, p.difficulty, p.url
         FROM problem_state ps
         JOIN problems p ON ps.problem_id = p.id
         WHERE ps.next_review_ts <= ?
         ORDER BY ps.next_review_ts ASC
         LIMIT 1",
            [now_ts],
            |row| {
                let id: i64 = row.get(0)?;
                Ok(ProblemView {
                    id,
                    title: row.get(1)?,
                    difficulty: row.get(2)?,
                    url: row.get(3)?,
                    track_name: "🧠 Spaced Review".to_string(), // Updated Label
                    skills: Vec::new(),                         // Placeholder, filled below
                })
            },
        )
        .optional()?;

    if let Some(mut p) = result {
        p.skills = get_skill_names_for_problem(conn, p.id).unwrap_or_default();
        debug!("[DB] Found due review: {}", p.title);
        return Ok(Some(p));
    }
    Ok(None)
}

pub fn get_unlocked_skills(conn: &Connection) -> Result<Vec<i64>> {
    // A skill is unlocked if all its prerequisites are met.
    // Prereq met = (Mastery >= Unlock_Threshold) OR (Mastery >= Consolidation AND Attempts >= Consolidation)
    let mut stmt = conn.prepare("SELECT id FROM skills")?;
    let all_skills = stmt.query_map([], |row| row.get::<_, i64>(0))?;

    let mut unlocked = Vec::new();

    // Note: We are doing N+1 query here for logic clarity, but since N=18 (skills), it's negligible.
    // Can be optimized into a single complex query if scaling is needed.
    for s in all_skills {
        let sid = s?;
        let failed_prereqs: i64 = conn.query_row(
            "SELECT count(*) FROM skill_prereqs sp
             JOIN skill_state ss ON sp.prereq_id = ss.skill_id
             WHERE sp.skill_id = ?
             AND (ss.mastery < ? OR (ss.mastery < ? AND ss.attempts < ?))",
            params![
                sid,
                MASTERY_UNLOCK_THRESHOLD,
                MASTERY_CONSOLIDATION_THRESHOLD,
                ATTEMPTS_CONSOLIDATION_THRESHOLD
            ],
            |row| row.get(0),
        )?;

        if failed_prereqs == 0 {
            unlocked.push(sid);
        }
    }
    Ok(unlocked)
}

pub fn find_new_problem_for_skills(
    conn: &Connection,
    track_id: i64,
    skill_ids: &[i64],
) -> Result<Option<ProblemView>> {
    if skill_ids.is_empty() {
        return Ok(None);
    }

    let placeholders = skill_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Note: We use GROUP BY because a problem might map to multiple unlocked skills.
    let sql = format!(
        "SELECT p.id, p.title, p.difficulty, p.url
         FROM problems p
         JOIN track_problems tp ON p.id = tp.problem_id
         JOIN problem_skills ps ON p.id = ps.problem_id
         WHERE tp.track_id = ?
         AND ps.skill_id IN ({})
         -- Exclude if the problem itself is tracked
         AND p.id NOT IN (SELECT problem_id FROM problem_state)
         -- Exclude if the problem is an alternative to a tracked parent
         AND p.id NOT IN (
            SELECT id FROM alternatives 
            WHERE parent_id IN (SELECT problem_id FROM problem_state)
         )
         -- Exclude if the problem IS the parent of a tracked alternative
         AND p.id NOT IN (
            SELECT parent_id FROM alternatives
            WHERE id IN (SELECT problem_id FROM attempts)
         )
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
    for id in skill_ids {
        params.push(Box::new(*id));
    }

    let result = conn
        .query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
            Ok(ProblemView {
                id: row.get(0)?,
                title: row.get(1)?,
                difficulty: row.get(2)?,
                url: row.get(3)?,
                track_name: "✨ New Discovery".to_string(),
                skills: Vec::new(),
            })
        })
        .optional()?;

    if let Some(mut p) = result {
        p.skills = get_skill_names_for_problem(conn, p.id).unwrap_or_default();
        return Ok(Some(p));
    }
    Ok(None)
}

pub fn find_cram_problem(
    conn: &Connection,
    track_id: i64,
    skill_ids: &[i64],
) -> Result<Option<ProblemView>> {
    if skill_ids.is_empty() {
        return Ok(None);
    }

    let placeholders = skill_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Note: We do NOT group by here intentionally.
    // If a problem uses a low-mastery skill, we want that specific row to bubble up
    // via the ORDER BY mastery clause.
    let sql = format!(
        "SELECT p.id, p.title, p.difficulty, p.url
         FROM problems p
         JOIN track_problems tp ON p.id = tp.problem_id
         JOIN problem_skills ps ON p.id = ps.problem_id
         JOIN skill_state ss ON ps.skill_id = ss.skill_id
         WHERE tp.track_id = ?
         AND ps.skill_id IN ({}) 
         ORDER BY ss.mastery ASC, RANDOM()
         LIMIT 1",
        placeholders
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    params.push(Box::new(track_id));
    for id in skill_ids {
        params.push(Box::new(*id));
    }

    let result = conn
        .query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
            Ok(ProblemView {
                id: row.get(0)?,
                title: row.get(1)?,
                difficulty: row.get(2)?,
                url: row.get(3)?,
                track_name: "🔥 Cram Mode".to_string(),
                skills: Vec::new(),
            })
        })
        .optional()?;

    if let Some(mut p) = result {
        p.skills = get_skill_names_for_problem(conn, p.id).unwrap_or_default();
        return Ok(Some(p));
    }
    Ok(None)
}
