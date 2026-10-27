use crate::components::navbar::ActiveTab;
use crate::models::{AuthSession, TableData, format_date_de, format_euros};
use crate::storage::{SupabaseConfig, delete_session_from_supabase, is_auth_expired_error, save_local_data};
use leptos::prelude::*;
use uuid::Uuid;

#[component]
pub fn HistoryView(
    table_data: ReadSignal<TableData>,
    set_table_data: WriteSignal<TableData>,
    config: ReadSignal<SupabaseConfig>,
    auth: ReadSignal<AuthSession>,
    set_editing_session_id: WriteSignal<Option<Uuid>>,
    set_active_tab: WriteSignal<ActiveTab>,
    set_sync_status: WriteSignal<String>,
) -> impl IntoView {
    let delete_session = move |session_id: Uuid| {
        let cur_auth = auth.get();
        if !cur_auth.can_manage_table() {
            return;
        }
        let mut updated = table_data.get();
        updated.sessions.retain(|s| s.id != session_id);
        save_local_data(&updated);
        set_table_data.set(updated);

        let cfg = config.get();
        set_sync_status.set("Lösche…".to_string());
        wasm_bindgen_futures::spawn_local(async move {
            match delete_session_from_supabase(&cfg, &cur_auth, session_id).await {
                Ok(()) => {
                    if cfg.is_configured() {
                        set_sync_status.set("Cloud & Lokal synchron".to_string());
                    } else {
                        set_sync_status.set("Lokal gespeichert".to_string());
                    }
                }
                Err(e) => {
                    if is_auth_expired_error(&e) {
                        set_sync_status.set(e);
                    } else {
                        set_sync_status.set(format!("Lösch-Fehler: {e}"));
                    }
                }
            }
        });
    };

    let edit_session = move |session_id: Uuid| {
        set_editing_session_id.set(Some(session_id));
        set_active_tab.set(ActiveTab::LogSession);
    };

    view! {
        <section class="history-section">
            <div class="card">
                <div class="card-header">
                    <h2>"📜 Gespielte Schafkopf-Abende"</h2>
                    <span class="card-subtitle">
                        {move || format!("{} Abende dokumentiert", table_data.get().sessions.len())}
                    </span>
                </div>

                {move || {
                    let data = table_data.get();
                    let cur_auth = auth.get();
                    let sessions = data.sessions_newest_first();

                    if sessions.is_empty() {
                        return view! {
                            <div class="empty-state">
                                <p>"Noch keine Spielabende vorhanden."</p>
                            </div>
                        }.into_any();
                    }

                    view! {
                        <div class="history-list">
                            {sessions.into_iter().map(|session| {
                                let sid = session.id;
                                let date_str = format_date_de(&session.played_at);
                                let dur_str = session
                                    .duration_minutes
                                    .map(|m| format!("{:.1} Std.", (m as f64) / 60.0))
                                    .unwrap_or_else(|| "–".to_string());
                                let note_str = session.note.clone();
                                let non_zero_diff = (!session.is_zero_sum()).then(|| session.total_balance_cents());

                                let pills = session.results.iter().map(|res| {
                                    let p_name = data
                                        .player_by_id(&res.player_id)
                                        .map(|p| p.name.clone())
                                        .unwrap_or_else(|| "Unbekannt".to_string());
                                    let p_color = data
                                        .player_by_id(&res.player_id)
                                        .map(|p| p.avatar_color.clone())
                                        .unwrap_or_else(|| "#94a3b8".to_string());
                                    let cents = res.amount_cents;

                                    view! {
                                        <div class="result-pill">
                                            <span class="color-dot" style=format!("background-color: {p_color};")></span>
                                            <span class="pill-name">{p_name}</span>
                                            <strong class={if cents >= 0 { "pos-text" } else { "neg-text" }}>
                                                {format_euros(cents, true)}
                                            </strong>
                                        </div>
                                    }
                                }).collect_view();

                                view! {
                                    <article class="history-item">
                                        <div class="history-meta">
                                            <div class="history-date-group">
                                                <strong class="history-date">{date_str}</strong>
                                                <span class="history-duration">{dur_str}</span>
                                                {(!note_str.is_empty()).then(|| view! {
                                                    <span class="history-note">{note_str}</span>
                                                })}
                                                {non_zero_diff.map(|diff| view! {
                                                    <span class="history-note">{format!("Diff: {}", format_euros(diff, true))}</span>
                                                })}
                                            </div>
                                            <div class="history-actions">
                                                <button
                                                    type="button"
                                                    class="ghost-btn"
                                                    title=if cur_auth.is_admin { "Alle Ergebnisse bearbeiten (Admin)" } else { "Eigenes Ergebnis für diesen Abend eintragen/ändern" }
                                                    on:click=move |_| edit_session(sid)
                                                >
                                                    {if cur_auth.is_admin { "✏️ Abend bearbeiten" } else { "✏️ Mein Ergebnis ändern" }}
                                                </button>
                                                {cur_auth.can_manage_table().then(|| view! {
                                                    <button
                                                        type="button"
                                                        class="danger-ghost-btn"
                                                        title="Abend löschen (Nur Admin)"
                                                        on:click=move |_| delete_session(sid)
                                                    >
                                                        "🗑️ Löschen"
                                                    </button>
                                                })}
                                            </div>
                                        </div>

                                        <div class="history-results">
                                            {pills}
                                        </div>
                                    </article>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>
        </section>
    }
}
