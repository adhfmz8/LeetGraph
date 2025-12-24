// src/constants.rs

// --- Time Constants ---
pub const DAY_SECONDS: i64 = 86400;
pub const EXPECTED_TIME_EASY: f64 = 10.0; // Minutes
pub const EXPECTED_TIME_MEDIUM: f64 = 25.0; // Minutes
pub const EXPECTED_TIME_HARD: f64 = 45.0; // Minutes

// --- Spaced Repetition (SM-2) Parameters ---
pub const ALPHA: f64 = 0.15; // Mastery gain per solve
pub const INTERVAL_MIN: f64 = 1.0; // Days
pub const INTERVAL_MAX: f64 = 180.0; // Days

pub const EASE_FACTOR_MIN: f64 = 1.3;
pub const EASE_FACTOR_MAX: f64 = 5.0;
pub const EASE_FACTOR_DEFAULT: f64 = 2.5;

// Adjustments
pub const EASE_FACTOR_DECREMENT_FAIL: f64 = 0.20;
pub const EASE_FACTOR_DECREMENT_STRUGGLE: f64 = 0.15;
pub const EASE_FACTOR_INCREMENT_CLEAN: f64 = 0.15;
pub const EASE_FACTOR_INCREMENT_SPEED: f64 = 0.15;
pub const EASE_FACTOR_NEUTRAL_GRIT: f64 = 0.05;

// Interval Multipliers
pub const INTERVAL_NEW_GRIT: f64 = 2.0;
pub const INTERVAL_NEW_CLEAN: f64 = 4.0;
pub const INTERVAL_MULTIPLIER_STRUGGLE: f64 = 0.7;
pub const INTERVAL_MULTIPLIER_SPEED: f64 = 1.2;

// --- Skill Tree / Mastery ---
pub const MASTERY_UNLOCK_THRESHOLD: f64 = 0.7;
pub const MASTERY_CONSOLIDATION_THRESHOLD: f64 = 0.9;
pub const ATTEMPTS_CONSOLIDATION_THRESHOLD: i64 = 2;

pub const DIFFICULTY_MULTIPLIER_EASY: f64 = 0.8;
pub const DIFFICULTY_MULTIPLIER_MEDIUM: f64 = 1.2;
pub const DIFFICULTY_MULTIPLIER_HARD: f64 = 1.5;

pub const PERFORMANCE_MULTIPLIER_FAIL: f64 = 0.0;
pub const PERFORMANCE_MULTIPLIER_NEW_GRIT: f64 = 1.2;
pub const PERFORMANCE_MULTIPLIER_NEW_CLEAN: f64 = 1.0;
pub const PERFORMANCE_MULTIPLIER_REVIEW: f64 = 0.3;
