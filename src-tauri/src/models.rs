// src/models.rs

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Mutex;

// --- App State ---

pub struct AppState {
    pub db: Mutex<Connection>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        AppState {
            db: Mutex::new(conn),
        }
    }
}

// --- Data Models ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Difficulty {
    Easy = 1,
    Medium = 2,
    Hard = 3,
}

impl Difficulty {
    pub fn as_str(&self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
        }
    }
}

impl FromStr for Difficulty {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Easy" => Ok(Difficulty::Easy),
            "Medium" => Ok(Difficulty::Medium),
            "Hard" => Ok(Difficulty::Hard),
            _ => Ok(Difficulty::Medium), // Default fallback
        }
    }
}

impl ToString for Difficulty {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProblemView {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub difficulty: String,
    pub track_name: String,
}

#[derive(Deserialize, Debug)]
pub struct AttemptLog {
    pub problem_id: i64,
    pub time_minutes: f64,
    pub solved: bool,
    pub read_solution: bool,
}

// Used for seeding
#[derive(Deserialize)]
pub struct JsonProblem {
    pub id: i64,
    pub title: String,
    pub difficulty: String,
    pub category: String,
    pub url: String,
    #[serde(default)]
    pub alternatives: Vec<JsonAlternative>,
}

//  struct for the nested data
#[derive(Deserialize)]
pub struct JsonAlternative {
    pub id: i64,
    pub title: String,
    pub difficulty: String,
    pub url: String,
}

// Internal State Models
pub struct ProblemRepetitionState {
    pub problem_id: i64,
    pub ease_factor: f64,
    pub interval_days: f64,
    pub next_review_ts: i64,
}

pub struct SkillMasteryState {
    pub skill_id: i64,
    pub mastery: f64,
    pub attempts: i32,
}
