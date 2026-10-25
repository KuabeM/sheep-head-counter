use crate::components::charts::{CumulativeBankrollChart, PlayerSessionBarsChart};
use crate::components::navbar::ActiveTab;
use crate::models::{TableData, compute_all_stats, format_euros};
use leptos::prelude::*;
use uuid::Uuid;

#[component]
pub fn Dashboard(
    table_data: ReadSignal<TableData>,
    set_active_tab: WriteSignal<ActiveTab>,
) -> impl IntoView {
    let (focused_player, set_focused_player) = signal::<Option<Uuid>>(None);

    let stats = Signal::derive(move || compute_all_stats(&table_data.get()));

    // Ensure a default player is highlighted for the personal KPI cards (first player or Korbinian)
    let active_player_stats = Signal::derive(move || {
        let all = stats.get();
        if let Some(id) = focused_player.get() {
            if let Some(found) = all.iter().find(|s| s.player_id == id) {
                return Some(found.clone());
            }
        }
        // Default to "Korbinian" if present, else leader
        all.iter()
            .find(|s| s.player_name.eq_ignore_ascii_case("korbinian"))
            .cloned()
            .or_else(|| all.first().cloned())
    });

    view! {
        <section class="dashboard-view">
            // Top bar: Player perspective selector + Quick Action
            <div class="dashboard-toolbar">
                <div class="perspective-selector" role="group" aria-label="Spieler-Fokus wählen">
                    <span class="toolbar-label">"Spieler-Fokus:"</span>
                    <button
                        type="button"
                        class=move || if focused_player.get().is_none() { "pill-btn active" } else { "pill-btn" }
                        on:click=move |_| set_focused_player.set(None)
                    >
                        "Alle vergleichen"
                    </button>
                    {move || {
                        table_data.get().players.into_iter().map(|p| {
                            let pid = p.id;
                            let color = p.avatar_color.clone();
                            view! {
                                <button
                                    type="button"
                                    class=move || if focused_player.get() == Some(pid) { "pill-btn active" } else { "pill-btn" }
                                    on:click=move |_| {
                                        if focused_player.get() == Some(pid) {
                                            set_focused_player.set(None);
                                        } else {
                                            set_focused_player.set(Some(pid));
                                        }
                                    }
                                >
                                    <span class="color-dot" style=format!("background-color: {color};")></span>
                                    {p.name}
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>

                <button
                    type="button"
                    class="primary-btn"
                    on:click=move |_| set_active_tab.set(ActiveTab::LogSession)
                >
                    "➕ Neuen Abend eintragen"
                </button>
            </div>

            // KPI Cards for the selected / default player
            {move || {
                let Some(st) = active_player_stats.get() else {
                    return view! { <p>"Keine Spieler angelegt."</p> }.into_any();
                };

                let win_pct = format!("{:.1} %", st.win_ratio_percent());
                let avg_session = format_euros(st.avg_cents_per_session().round() as i64, true);
                let hourly_str = st
                    .cents_per_hour()
                    .map(|c| format!("{}/Std.", format_euros(c.round() as i64, true)))
                    .unwrap_or_else(|| "–".to_string());
                let total_hours = (st.total_minutes as f64) / 60.0;

                let streak_str = if st.current_streak > 0 {
                    format!("🔥 {} Siege in Folge", st.current_streak)
                } else if st.current_streak < 0 {
                    format!("❄️ {} Niederlagen", st.current_streak.abs())
                } else {
                    "Neutral".to_string()
                };

                view! {
                    <div class="kpi-section">
                        <div class="kpi-section-title">
                            <span
                                class="player-badge"
                                style=format!("border-left: 4px solid {};", st.avatar_color)
                            >
                                {format!("Statistik für {}", st.player_name)}
                            </span>
                            <span class="kpi-sub">
                                {format!("{} Spielabende ({:.1} Std. gesamt)", st.sessions_played, total_hours)}
                            </span>
                        </div>

                        <div class="kpi-grid">
                            <div class="kpi-card">
                                <span class="kpi-label">"Gesamtbilanz"</span>
                                <span class={if st.total_cents >= 0 { "kpi-value pos-text" } else { "kpi-value neg-text" }}>
                                    {format_euros(st.total_cents, true)}
                                </span>
                                <span class="kpi-foot">{format!("Ø {} pro Abend", avg_session)}</span>
                            </div>

                            <div class="kpi-card">
                                <span class="kpi-label">"Siegquote (Abende)"</span>
                                <span class="kpi-value">{win_pct}</span>
                                <span class="kpi-foot">
                                    {format!("{} G • {} V • {} U", st.wins, st.losses, st.draws)}
                                </span>
                            </div>

                            <div class="kpi-card">
                                <span class="kpi-label">"Stundenlohn"</span>
                                <span class={if st.total_cents >= 0 { "kpi-value pos-text" } else { "kpi-value neg-text" }}>
                                    {hourly_str}
                                </span>
                                <span class="kpi-foot">{streak_str}</span>
                            </div>

                            <div class="kpi-card">
                                <span class="kpi-label">"Bester / Schlechtester Abend"</span>
                                <div class="kpi-split">
                                    <span class="pos-text">{format_euros(st.best_session_cents, true)}</span>
                                    <span class="divider">"/"</span>
                                    <span class="neg-text">{format_euros(st.worst_session_cents, true)}</span>
                                </div>
                                <span class="kpi-foot">"Rekord-Abende"</span>
                            </div>
                        </div>
                    </div>
                }.into_any()
            }}

            // Charts Grid
            <div class="charts-grid">
                <CumulativeBankrollChart stats=stats focused_player=focused_player />
                <PlayerSessionBarsChart stats=stats focused_player=focused_player />
            </div>

            // Full Table Leaderboard
            <div class="card leaderboard-card">
                <div class="card-header">
                    <h3>"🏆 Ewige Tabelle (Alle Spieler)"</h3>
                    <span class="card-subtitle">"Klicke auf einen Spieler für die Detailansicht"</span>
                </div>
                <div class="table-responsive">
                    <table class="stats-table">
                        <thead>
                            <tr>
                                <th>"Rang"</th>
                                <th>"Spieler"</th>
                                <th class="num-col">"Abende"</th>
                                <th class="num-col">"Bilanz (S/N)"</th>
                                <th class="num-col">"Siegquote"</th>
                                <th class="num-col">"Ø / Abend"</th>
                                <th class="num-col">"€ / Stunde"</th>
                                <th class="num-col">"Gesamt (€)"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                stats.get().into_iter().enumerate().map(|(idx, st)| {
                                    let pid = st.player_id;
                                    let is_selected = focused_player.get() == Some(pid);
                                    let rank_medal = match idx {
                                        0 => "🥇 1.".to_string(),
                                        1 => "🥈 2.".to_string(),
                                        2 => "🥉 3.".to_string(),
                                        n => format!("{}.", n + 1),
                                    };

                                    let hourly = st
                                        .cents_per_hour()
                                        .map(|c| format_euros(c.round() as i64, true))
                                        .unwrap_or_else(|| "–".to_string());

                                    view! {
                                        <tr
                                            class={if is_selected { "clickable-row selected-row" } else { "clickable-row" }}
                                            on:click=move |_| set_focused_player.set(Some(pid))
                                        >
                                            <td class="rank-cell">{rank_medal}</td>
                                            <td>
                                                <div class="player-cell">
                                                    <span
                                                        class="color-dot"
                                                        style=format!("background-color: {};", st.avatar_color)
                                                    ></span>
                                                    <strong>{st.player_name.clone()}</strong>
                                                </div>
                                            </td>
                                            <td class="num-col">{st.sessions_played}</td>
                                            <td class="num-col">{format!("{}W – {}L", st.wins, st.losses)}</td>
                                            <td class="num-col">{format!("{:.1} %", st.win_ratio_percent())}</td>
                                            <td class="num-col">
                                                {format_euros(st.avg_cents_per_session().round() as i64, true)}
                                            </td>
                                            <td class="num-col">{hourly}</td>
                                            <td class={if st.total_cents >= 0 { "num-col total-cell pos-text" } else { "num-col total-cell neg-text" }}>
                                                {format_euros(st.total_cents, true)}
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()
                            }}
                        </tbody>
                    </table>
                </div>
            </div>
        </section>
    }
}
