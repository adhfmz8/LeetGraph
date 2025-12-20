# LeetCode Trainer (Tauri + NeetCode 150)

A specialized spaced-repetition desktop application designed to help you master Data Structures and Algorithms. Unlike standard flashcard apps, this trainer uses a **Skill Dependency Graph** and a custom **"Grit vs. Recall" algorithm** to ensure you learn concepts in the correct order and review them at optimal intervals.

Built with **Rust (Tauri)** and **Vanilla JS**.

## Features

* **Smart Pedagogy:**
  * **Skill Trees:** You cannot get "Two Pointers" problems until you demonstrate mastery in "Arrays & Hashing". The app enforces a Directed Acyclic Graph (DAG) of prerequisites.
  * **Spaced Repetition System (SRS):** A modified SM-2 algorithm tailored for coding. It distinguishes between "Grit" (struggling but solving) and "Recall" (muscle memory).
* **Privacy First:** All data is stored locally in a SQLite database (`neetcode_trainer.db`).
* **NeetCode 150 Integration:** Pre-loaded with the NeetCode 150 roadmap.
* **Distraction-Free UI:** A clean interface to focus on one problem at a time.

## Building form Source

Currently, there are no pre-built binaries available. You can build the application yourself using the Tauri CLI.

### Prerequisites

1. **Rust:** Install via [rustup](https://rustup.rs/).
2. **Node.js:** Install via [nodejs.org](https://nodejs.org/).
3. **OS Dependencies:** Follow the [Tauri Prerequisites Guide](https://tauri.app/v1/guides/getting-started/prerequisites) for your specific OS (Windows, macOS, or Linux).

### Installation Steps

1. **Clone the repository:**

    ```bash
    git clone https://github.com/adhfmz8/LeetGraph.git
    cd LeetGraph
    ```

2. **Install frontend dependencies:**

    ```bash
    npm install
    # or
    yarn install
    ```

Optional: **Run in Development Mode:**
    To test the app locally with hot-reloading:

    ```bash
    npm run tauri dev
    ```

3. **Build Release Artifact:**
    To create an optimized executable for your machine:

    ```bash
    npm run tauri build
    ```

    * **Windows:** The `.exe` will be in `src-tauri/target/release/bundle/msi/` or `nsis/`.
    * **Mac:** The `.app` or `.dmg` will be in `src-tauri/target/release/bundle/macos/` or `dmg/`.
    * **Linux:** Look in `src-tauri/target/release/bundle/deb/` or `appimage/`.

## How to Use

The application acts as your personal coach. It decides **what** you should do and **when**.

### 1. The Dashboard

When you open the app, it automatically fetches the most important problem for you based on this priority queue:

1. **Reviews:** Problems you are about to forget (Memory Protection).
2. **Discovery:** New problems from unlocked categories (Skill Expansion).
3. **Cram/Grind:** If you are caught up, it picks problems from your weakest unlocked areas.

### 2. Solving a Problem

1. Click **"Open Problem"** to view the challenge in your browser (LeetCode).
2. Solve the problem in your IDE or browser.
3. **Time yourself!** The algorithm cares about how long it took relative to the difficulty.

### 3. Logging the Attempt

Back in the app, fill out the Session Log:

* **Time Spent:** Be honest.
* **Solved:** Check this if your code passed all test cases.
* **Viewed Hints:** Check this if you looked at the solution, hints, or comments.

### 4. The Algorithm Logic

When you click **"Log & Next Problem"**, the backend calculates your next interval:

| Scenario | Outcome |
| :--- | :--- |
| **Failed / Looked at Solution** | **Reset.** You will see this problem again tomorrow. Mastery decreases. |
| **New Problem + Slow Time** | **Grit Bonus.** You struggled but solved it. Review in 2 days to consolidate. |
| **New Problem + Fast Time** | **Clean Solve.** Review in 4 days. |
| **Review + Slow Time** | **Struggle Review.** Interval shrinks (you almost forgot it). |
| **Review + Fast Time** | **Speed Review.** Interval expands significantly (strong muscle memory). |

## Skill Dependency Tree

The app prevents you from jumping into advanced topics before mastering the basics. The internal graph looks like this:

* **Arrays & Hashing** → Unlocks *Two Pointers* & *Stack*
* **Two Pointers** → Unlocks *Binary Search*, *Sliding Window*, *Linked List*
* **Tree** → Unlocks *Tries*, *Heap*, *Backtracking*
* ...and so on.

To unlock a new tree, you must reach **70% mastery** in the prerequisite skill.

## Data Location

Your progress is saved in a local SQLite database found at:

* **Windows:** `C:\Users\<User>\AppData\Roaming\com.neetcode.trainer\neetcode_trainer.db`
* **Mac:** `~/Library/Application Support/com.neetcode.trainer/neetcode_trainer.db`
* **Linux:** `~/.config/com.neetcode.trainer/neetcode_trainer.db`

## Contributing

This is a personal learning tool, but contributions are welcome!

1. Fork the repo.
2. Create a feature branch (`git checkout -b feature/amazing-feature`).
3. Commit your changes.
4. Open a Pull Request.
