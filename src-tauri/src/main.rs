// src/main.rs

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod constants;
mod database;
mod models;
mod pedagogy;
mod repository;

use crate::models::{AppState, AttemptLog, ProblemView};
use rusqlite::Connection;
use std::fs;
use tauri::{Manager, State};

use log::{debug, error, info};

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    webbrowser::open(&url).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_next_problem(state: State<AppState>) -> Result<Option<ProblemView>, String> {
    let conn = state.db.lock().unwrap();
    pedagogy::get_next_problem(&conn)
}

#[tauri::command]
fn submit_attempt(state: State<AppState>, log: AttemptLog) -> Result<(), String> {
    let conn = state.db.lock().unwrap();
    pedagogy::process_attempt(&conn, &log)
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    info!("Starting NeetCode Trainer Backend...");
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            let app_data_dir = app_handle
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");

            if !app_data_dir.exists() {
                fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            }

            let db_path = app_data_dir.join("neetcode_trainer.db");
            info!("Database path: {:?}", db_path);
            let conn = Connection::open(db_path).expect("Failed to open DB");

            // Init Database (Schema + Seeds)
            database::init_db(&conn).expect("Failed to init DB");

            app.manage(AppState::new(conn));
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
