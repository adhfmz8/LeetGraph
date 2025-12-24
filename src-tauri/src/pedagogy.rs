// src/pedagogy.rs

use crate::constants::*;
use crate::models::{AttemptLog, Difficulty, ProblemView};
use crate::repository;
use chrono::Utc;
use log::{debug, info, warn};
use rusqlite::Connection;

// --- Public Interface ---

pub fn get_next_problem(conn: &Connection) -> Result<Option<ProblemView>, String> {
    let now = Utc::now().timestamp();
    let track_id = 1; // Default track
    debug!("Requesting next problem...");

    // 1. Review (Memory Protection)
    if let Ok(Some(p)) = repository::find_due_review(conn, now) {
        info!("Serving Due Review: {} (ID: {})", p.title, p.id);
        return Ok(Some(p));
    }

    // 2. Discovery (New unlocked problems)
    // We fetch unlocked skills first
    let unlocked_skills = repository::get_unlocked_skills(conn).map_err(|e| e.to_string())?;
    debug!("Unlocked Skill IDs: {:?}", unlocked_skills);

    if let Ok(Some(p)) = repository::find_new_problem_for_skills(conn, track_id, &unlocked_skills) {
        info!("Serving Discovery: {} (ID: {})", p.title, p.id);
        return Ok(Some(p));
    }

    // 3. Cram (Grind lowest mastery)
    if let Ok(Some(p)) = repository::find_cram_problem(conn, track_id) {
        warn!(
            "No new content/reviews available. Entering Cram Mode: {} (ID: {})",
            p.title, p.id
        );
        return Ok(Some(p));
    }

    info!("No problems available.");
    Ok(None)
}

pub fn process_attempt(conn: &Connection, log: &AttemptLog) -> Result<(), String> {
    let now = Utc::now().timestamp();
    info!("Processing attempt for Problem ID: {}", log.problem_id);

    let (difficulty, skill_ids) =
        repository::get_problem_metadata(conn, log.problem_id).map_err(|e| e.to_string())?;
    let prior_attempts =
        repository::get_attempt_count(conn, log.problem_id).map_err(|e| e.to_string())?;

    // 1. Log Attempt
    repository::log_attempt(
        conn,
        log.problem_id,
        log.time_minutes,
        log.solved,
        log.read_solution,
        now,
    )
    .map_err(|e| e.to_string())?;

    // 2. Update Repetition State (SM-2 Logic)
    update_repetition_logic(conn, log, difficulty, prior_attempts, now)?;

    // 3. Update Skill Mastery
    update_mastery_logic(conn, log, difficulty, &skill_ids)?;

    Ok(())
}

// --- Internal Algorithm Logic ---

fn update_repetition_logic(
    conn: &Connection,
    log: &AttemptLog,
    difficulty: Difficulty,
    prior_attempts: i64,
    now: i64,
) -> Result<(), String> {
    let mut state = repository::get_problem_repetition_state(conn, log.problem_id)
        .map_err(|e| e.to_string())?;

    // Snapshot old state for logging
    let old_ease = state.ease_factor;
    let old_interval = state.interval_days;

    let is_new = prior_attempts <= 0; // Since we just logged one, current count is 1+; check is based on *before* this attempt
    let expected_time = match difficulty {
        Difficulty::Easy => EXPECTED_TIME_EASY,
        Difficulty::Medium => EXPECTED_TIME_MEDIUM,
        Difficulty::Hard => EXPECTED_TIME_HARD,
    };
    let time_ratio = log.time_minutes / expected_time;
    let is_fail = !log.solved || log.read_solution;

    debug!(
        "[SM-2 Input] New: {}, Fail: {}, TimeRatio: {:.2}, Diff: {:?}",
        is_new, is_fail, time_ratio, difficulty
    );

    if is_fail {
        state.ease_factor = (state.ease_factor - EASE_FACTOR_DECREMENT_FAIL).max(EASE_FACTOR_MIN);
        state.interval_days = INTERVAL_MIN;
    } else if is_new {
        if time_ratio > 1.5 {
            // Grit solve (took long)
            debug!("[SM-2 logic] Branch: New Grit");
            state.ease_factor -= EASE_FACTOR_NEUTRAL_GRIT;
            state.interval_days = INTERVAL_NEW_GRIT;
        } else {
            // Clean solve
            debug!("[SM-2 logic] Branch: New Clean");
            state.ease_factor += EASE_FACTOR_INCREMENT_CLEAN;
            state.interval_days = INTERVAL_NEW_CLEAN;
        }
    } else {
        // Review
        if time_ratio > 2.0 {
            // Struggle
            debug!("[SM-2 logic] Branch: Review Struggle");
            state.ease_factor -= EASE_FACTOR_DECREMENT_STRUGGLE;
            state.interval_days *= INTERVAL_MULTIPLIER_STRUGGLE;
        } else if time_ratio < 0.6 {
            // Speed
            debug!("[SM-2 logic] Branch: Review Speed");
            state.ease_factor += EASE_FACTOR_INCREMENT_SPEED;
            state.interval_days *= state.ease_factor * INTERVAL_MULTIPLIER_SPEED;
        } else {
            // Normal
            debug!("[SM-2 logic] Branch: Review Normal");
            state.interval_days *= state.ease_factor;
        }
    }

    // Clamping
    state.ease_factor = state.ease_factor.clamp(EASE_FACTOR_MIN, EASE_FACTOR_MAX);
    state.interval_days = state.interval_days.clamp(INTERVAL_MIN, INTERVAL_MAX);
    state.next_review_ts = now + ((state.interval_days * DAY_SECONDS as f64) as i64);

    info!(
        "[SM-2 Result] Problem {}: Ease {:.2} -> {:.2}, Interval {:.1}d -> {:.1}d",
        log.problem_id, old_ease, state.ease_factor, old_interval, state.interval_days
    );

    repository::save_problem_repetition_state(conn, &state).map_err(|e| e.to_string())?;
    Ok(())
}

fn update_mastery_logic(
    conn: &Connection,
    log: &AttemptLog,
    difficulty: Difficulty,
    skill_ids: &[i64],
) -> Result<(), String> {
    let diff_mult = match difficulty {
        Difficulty::Easy => DIFFICULTY_MULTIPLIER_EASY,
        Difficulty::Medium => DIFFICULTY_MULTIPLIER_MEDIUM,
        Difficulty::Hard => DIFFICULTY_MULTIPLIER_HARD,
    };

    let expected_time = match difficulty {
        Difficulty::Easy => EXPECTED_TIME_EASY,
        Difficulty::Medium => EXPECTED_TIME_MEDIUM,
        Difficulty::Hard => EXPECTED_TIME_HARD,
    };
    let time_ratio = log.time_minutes / expected_time;
    let is_fail = !log.solved || log.read_solution;

    // We assume it's "New" for performance bonus if it was the first solve,
    // but calculating exact "newness" here for mastery is slightly fuzzy in this arch.
    // For simplicity, we trust the ratio/outcome more than strict history count here.

    let perf_mult = if is_fail {
        PERFORMANCE_MULTIPLIER_FAIL
    } else if time_ratio > 1.5 {
        // Assume Grit context
        PERFORMANCE_MULTIPLIER_NEW_GRIT
    } else {
        // Clean or Review (Review gets penalized, but here we simplify to prevent complex history lookup just for this multiplier)
        // If we want strict review penalty, we need to pass `is_new` down.
        // Assuming "New Clean" as baseline for success, and "Review" needs handling:
        // *Refinement*: If it's a review, we should use PERFORMANCE_MULTIPLIER_REVIEW.
        // Let's check attempt count via repository again or pass it down.
        let attempts =
            repository::get_attempt_count(conn, log.problem_id).map_err(|e| e.to_string())?;
        if attempts > 1 {
            PERFORMANCE_MULTIPLIER_REVIEW
        } else {
            PERFORMANCE_MULTIPLIER_NEW_CLEAN
        }
    };

    let delta = ALPHA * diff_mult * perf_mult;
    debug!(
        "[Mastery Input] Delta calculated: {:.4} (based on perf_mult: {:.2})",
        delta, perf_mult
    );

    for &sid in skill_ids {
        let mut s_state = repository::get_skill_state(conn, sid).map_err(|e| e.to_string())?;
        let old_mastery = s_state.mastery;
        s_state.mastery = (s_state.mastery + delta).clamp(0.0, 1.0);
        s_state.attempts += 1;

        info!(
            "[Mastery Result] Skill {}: {:.3} -> {:.3} (Attempts: {})",
            sid, old_mastery, s_state.mastery, s_state.attempts
        );
        repository::update_skill_state(conn, &s_state).map_err(|e| e.to_string())?;
    }

    Ok(())
}
