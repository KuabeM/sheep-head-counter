# ♠ Schafkopf Kasse — Session & Money Tracker

A fast, zero-cost web application built with **Rust (Leptos CSR + WebAssembly)** for tracking your
Schafkopf evenings, win ratios, hourly rates, and cumulative bankroll over time across all players.

## Features
- **Dashboard & Leaderboard**:
  - Personal & table-wide KPIs: Total Net Money (`€`), Evening Win Ratio (`%` and `W - L - D`),
    Hourly Rate (`€ / Std.`), Average per Evening, Current Streak, and Best/Worst Evening.
  - **Interactive SVG Charts**: Multi-player cumulative bankroll line chart over time + individual
    session win/loss bar chart.
- **Fast Evening Logger ("Abend eintragen")**:
  - Log the date, hours played, optional note, and each player's end-of-night net amount in Euros.
  - **1-Click Zero-Sum Calculator (`⚖️ Ausgleichen`)**: Automatically fills the remaining player's
    amount so the table sums to `0,00 €`.
- **100% Free Hosting & Storage**:
  - Works out of the box using **LocalStorage** (offline-capable).
  - Optional shared table sync via **Supabase Free Tier** protected by a shared table passphrase
    (verified inside Postgres via `pgcrypto`).
  - Includes a scheduled GitHub Action
    ([.github/workflows/keepalive.yml](file:///home/korbinian/RustProjects/sheep-head-counter/.github/workflows/keepalive.yml))
    that pings Supabase twice a week so the free-tier database never pauses.

---

## Local Development

1. Ensure you have the WASM target and `trunk` installed:
   ```bash
   rustup target add wasm32-unknown-unknown
   cargo install trunk
   ```
2. Start the live-reload dev server:
   ```bash
   trunk serve --open
   ```
3. Run the unit tests:
   ```bash
   cargo test
   ```

---

## Free Deployment Setup (GitHub Pages + Supabase)

### 1. Supabase Free Tier (Optional — for shared table sync)
1. Create a free project at [supabase.com](https://supabase.com).
2. Open the **SQL Editor** in Supabase and run the contents of
   [supabase/schema.sql](file:///home/korbinian/RustProjects/sheep-head-counter/supabase/schema.sql).
3. Copy your **Project URL** and **Publishable key** (`sb_publishable_...`) from *Project Settings →
   API Keys*.
4. Either enter them directly in the app under **🔑 Login & Spieler**, or add them as GitHub
   Repository Secrets (`SUPABASE_URL` and `SUPABASE_PUBLISHABLE_KEY`) so they are baked into your
   GitHub Pages build automatically.

### 2. GitHub Pages
1. In your GitHub repository, go to **Settings → Pages** and under **Build and deployment →
   Source**, select **GitHub Actions**.
2. Push to `main`. The workflow in
   [.github/workflows/deploy.yml](file:///home/korbinian/RustProjects/sheep-head-counter/.github/workflows/deploy.yml)
   will compile the Rust WASM bundle and deploy it to GitHub Pages.
