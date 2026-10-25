use crate::components::navbar::ActiveTab;
use crate::models::{
    AuthSession, GameSession, SessionResult, TableData, format_euros, parse_euros_to_cents,
};
use crate::storage::{SupabaseConfig, save_local_data, save_session_to_supabase};
use chrono::Local;
use leptos::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

#[component]
pub fn SessionForm(
    table_data: ReadSignal<TableData>,
    set_table_data: WriteSignal<TableData>,
    config: ReadSignal<SupabaseConfig>,
    auth: ReadSignal<AuthSession>,
    editing_session_id: ReadSignal<Option<Uuid>>,
    set_editing_session_id: WriteSignal<Option<Uuid>>,
    set_sync_status: WriteSignal<String>,
    set_active_tab: WriteSignal<ActiveTab>,
) -> impl IntoView {
    let existing_session = editing_session_id
        .get_untracked()
        .and_then(|sid| table_data.get_untracked().sessions.into_iter().find(|s| s.id == sid));

    let today_iso = Local::now().format("%Y-%m-%d").to_string();
    let init_date = existing_session
        .as_ref()
        .map(|s| s.played_at.clone())
        .unwrap_or(today_iso);
    let init_hours = existing_session
        .as_ref()
        .and_then(|s| s.duration_minutes)
        .map(|m| format!("{:.1}", (m as f64) / 60.0))
        .unwrap_or_else(|| "3.5".to_string());
    let init_note = existing_session
        .as_ref()
        .map(|s| s.note.clone())
        .unwrap_or_default();

    let (played_at, set_played_at) = signal(init_date);
    let (hours_input, set_hours_input) = signal(init_hours);
    let (note, set_note) = signal(init_note);
    let (allow_non_zero_sum, set_allow_non_zero_sum) = signal(!auth.get_untracked().is_admin);
    let (error_msg, set_error_msg) = signal::<Option<String>>(None);

    // Map of player_id -> (participating: bool, amount_str: String)
    let cur_auth = auth.get_untracked();
    let initial_map: HashMap<Uuid, (bool, String)> = table_data
        .get_untracked()
        .players
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            if let Some(ref ex) = existing_session {
                if let Some(cents) = ex.result_for_player(&p.id) {
                    let formatted = format!("{:.2}", (cents as f64) / 100.0).replace('.', ",");
                    return (p.id, (true, formatted));
                } else {
                    return (p.id, (false, String::new()));
                }
            }
            // For a new session: if Admin, check first 4 active players; if regular player, check own player
            let check = if cur_auth.is_admin {
                idx < 4 && p.active
            } else {
                cur_auth.claimed_player_id == Some(p.id)
            };
            (p.id, (check, String::new()))
        })
        .collect();

    let (player_inputs, set_player_inputs) = signal(initial_map);

    // Live sum of participating players in cents
    let current_sum_cents = Signal::derive(move || {
        let map = player_inputs.get();
        let mut sum = 0_i64;
        for (participating, val_str) in map.values() {
            if *participating {
                if let Some(cents) = parse_euros_to_cents(val_str) {
                    sum += cents;
                }
            }
        }
        sum
    });

    // Helper: Fill a specific player's input with the exact remainder so sum == 0
    let auto_balance_player = move |target_id: Uuid| {
        if !auth.get().can_edit_player(&target_id) {
            return;
        }
        set_player_inputs.update(|map| {
            let mut other_sum = 0_i64;
            for (pid, (part, val_str)) in map.iter() {
                if *pid != target_id && *part {
                    if let Some(c) = parse_euros_to_cents(val_str) {
                        other_sum += c;
                    }
                }
            }
            let remainder = -other_sum;
            let formatted = format!("{:.2}", (remainder as f64) / 100.0).replace('.', ",");
            if let Some(entry) = map.get_mut(&target_id) {
                entry.0 = true;
                entry.1 = formatted;
            }
        });
    };

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_error_msg.set(None);

        let cur_auth = auth.get();
        if cur_auth.claimed_player_id.is_none() {
            set_error_msg.set(Some(
                "Bitte wähle zuerst unter '🔑 Login & Spieler' dein Spielerprofil aus!".to_string(),
            ));
            return;
        }

        let map = player_inputs.get();
        let mut results = Vec::new();
        for player in &table_data.get().players {
            let can_edit = cur_auth.can_edit_player(&player.id);
            if let Some((true, val_str)) = map.get(&player.id) {
                if can_edit {
                    match parse_euros_to_cents(val_str) {
                        Some(cents) => {
                            results.push(SessionResult {
                                player_id: player.id,
                                amount_cents: cents,
                            });
                        }
                        None => {
                            set_error_msg.set(Some(format!(
                                "Ungültiger Euro-Betrag für {}. Bitte z.B. 12,50 oder -4,20 eingeben.",
                                player.name
                            )));
                            return;
                        }
                    }
                } else if let Some(ref ex) = existing_session {
                    // Keep existing read-only result for other players untouched
                    if let Some(old_cents) = ex.result_for_player(&player.id) {
                        results.push(SessionResult {
                            player_id: player.id,
                            amount_cents: old_cents,
                        });
                    }
                }
            }
        }

        if results.is_empty() {
            set_error_msg.set(Some(
                "Bitte mindestens dein eigenes Ergebnis eintragen.".to_string(),
            ));
            return;
        }

        let sum: i64 = results.iter().map(|r| r.amount_cents).sum();
        if cur_auth.is_admin && sum != 0 && !allow_non_zero_sum.get() {
            set_error_msg.set(Some(format!(
                "Die Summe aller Ergebnisse ist {} statt 0,00 €. Nutze '⚖️ Ausgleichen' oder aktiviere unten die Abweichungs-Option (falls noch nicht alle Spieler eingetragen haben).",
                format_euros(sum, true)
            )));
            return;
        }

        let duration_minutes = hours_input
            .get()
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|h| *h > 0.0)
            .map(|h| (h * 60.0).round() as u32);

        let session_id = editing_session_id.get().unwrap_or_else(Uuid::new_v4);
        let saved_session = GameSession {
            id: session_id,
            played_at: played_at.get(),
            duration_minutes,
            note: note.get().trim().to_string(),
            results,
        };

        let mut updated_data = table_data.get();
        if let Some(slot) = updated_data.sessions.iter_mut().find(|s| s.id == session_id) {
            *slot = saved_session.clone();
        } else {
            updated_data.sessions.push(saved_session.clone());
        }
        save_local_data(&updated_data);
        set_table_data.set(updated_data);
        set_editing_session_id.set(None);

        let cfg = config.get();
        set_sync_status.set("Speichere…".to_string());
        wasm_bindgen_futures::spawn_local(async move {
            match save_session_to_supabase(&cfg, &cur_auth, &saved_session).await {
                Ok(()) => {
                    if cfg.is_configured() {
                        set_sync_status.set("Cloud & Lokal synchron".to_string());
                    } else {
                        set_sync_status.set("Lokal gespeichert".to_string());
                    }
                }
                Err(e) => {
                    set_sync_status.set(format!("Sync-Fehler: {e}"));
                }
            }
        });

        set_active_tab.set(ActiveTab::Dashboard);
    };

    view! {
        <section class="form-section">
            <div class="card form-card">
                <div class="card-header">
                    <h2>
                        {move || if editing_session_id.get().is_some() {
                            "✏️ Schafkopf-Abend bearbeiten"
                        } else {
                            "➕ Schafkopf-Abend eintragen"
                        }}
                    </h2>
                    <span class="card-subtitle">
                        {move || {
                            let a = auth.get();
                            if a.is_admin {
                                "👑 Admin-Modus: Du kannst die Ergebnisse für alle Spieler eintragen und bearbeiten.".to_string()
                            } else if let Some(pid) = a.claimed_player_id {
                                let name = table_data.get().player_by_id(&pid).map(|p| p.name.clone()).unwrap_or_default();
                                format!("👤 Angemeldet als {name}: Du kannst deinen eigenen Gewinn/Verlust eintragen. Andere Spieler sind schreibgeschützt.")
                            } else {
                                "🔒 Bitte zuerst unter '🔑 Login & Spieler' dein Profil auswählen!".to_string()
                            }
                        }}
                    </span>
                </div>

                {move || auth.get().claimed_player_id.is_none().then(|| view! {
                    <div class="alert-error" style="margin-bottom: 1rem;">
                        "Du hast noch kein Spielerprofil ausgewählt. "
                        <button
                            type="button"
                            class="secondary-btn"
                            style="margin-left: 0.75rem;"
                            on:click=move |_| set_active_tab.set(ActiveTab::Settings)
                        >
                            "Jetzt Spielerprofil wählen →"
                        </button>
                    </div>
                })}

                <form on:submit=on_submit class="session-form">
                    <fieldset class="form-fieldset">
                        <legend>"1. Rahmendaten des Abends"</legend>
                        <div class="form-row-3">
                            <div class="form-group">
                                <label for="session-date">"Datum"</label>
                                <input
                                    type="date"
                                    id="session-date"
                                    name="session-date"
                                    required
                                    prop:value=move || played_at.get()
                                    on:input=move |ev| set_played_at.set(event_target_value(&ev))
                                />
                            </div>

                            <div class="form-group">
                                <label for="session-hours">"Spieldauer (Stunden)"</label>
                                <input
                                    type="text"
                                    inputmode="decimal"
                                    id="session-hours"
                                    name="session-hours"
                                    placeholder="z.B. 3,5"
                                    prop:value=move || hours_input.get()
                                    on:input=move |ev| set_hours_input.set(event_target_value(&ev))
                                />
                            </div>

                            <div class="form-group">
                                <label for="session-note">"Notiz / Ort / Tarif (optional)"</label>
                                <input
                                    type="text"
                                    id="session-note"
                                    name="session-note"
                                    placeholder="z.B. Beim Wirt, 10/20/50 Cent"
                                    prop:value=move || note.get()
                                    on:input=move |ev| set_note.set(event_target_value(&ev))
                                />
                            </div>
                        </div>
                    </fieldset>

                    <fieldset class="form-fieldset">
                        <legend>"2. Endergebnis pro Spieler (in Euro)"</legend>
                        <p class="fieldset-help">
                            "Positive Zahlen für Gewinn (z.B. " <code>"14,50"</code> "), negative für Verlust (z.B. " <code>"-6,20"</code> "). Du kannst nur dein eigenes Feld bearbeiten (der Admin kann alle Felder bearbeiten)."
                        </p>

                        <div class="player-inputs-list">
                            {move || {
                                let cur_auth = auth.get();
                                table_data.get().players.into_iter().map(|player| {
                                    let pid = player.id;
                                    let can_edit = cur_auth.can_edit_player(&pid);
                                    let p_name = player.name.clone();
                                    let p_color = player.avatar_color.clone();
                                    let input_id = format!("amount-{pid}");
                                    let check_id = format!("active-{pid}");

                                    let is_part = Signal::derive(move || {
                                        player_inputs.get().get(&pid).map(|x| x.0).unwrap_or(false)
                                    });
                                    let val_str = Signal::derive(move || {
                                        player_inputs.get().get(&pid).map(|x| x.1.clone()).unwrap_or_default()
                                    });

                                    view! {
                                        <div class=move || {
                                            if !can_edit {
                                                "player-input-row locked"
                                            } else if is_part.get() {
                                                "player-input-row active"
                                            } else {
                                                "player-input-row inactive"
                                            }
                                        }>
                                            <div class="player-check">
                                                <input
                                                    type="checkbox"
                                                    id=check_id.clone()
                                                    disabled=move || !can_edit
                                                    checked=move || is_part.get()
                                                    on:change=move |ev| {
                                                        if !can_edit { return; }
                                                        let checked = event_target_checked(&ev);
                                                        set_player_inputs.update(|map| {
                                                            if let Some(entry) = map.get_mut(&pid) {
                                                                entry.0 = checked;
                                                            }
                                                        });
                                                    }
                                                />
                                                <label for=check_id class="player-label">
                                                    <span class="color-dot" style=format!("background-color: {p_color};")></span>
                                                    {p_name}
                                                    {player.is_admin.then(|| view! { <span class="badge-admin">"👑 Admin"</span> })}
                                                    {(!can_edit).then(|| view! { <span class="badge-muted">"🔒 Nur Lesezugriff"</span> })}
                                                </label>
                                            </div>

                                            <div class="player-amount-controls">
                                                <div class="euro-input-wrap">
                                                    <input
                                                        type="text"
                                                        inputmode="decimal"
                                                        id=input_id
                                                        placeholder="0,00"
                                                        disabled=move || !can_edit || !is_part.get()
                                                        prop:value=move || val_str.get()
                                                        on:input=move |ev| {
                                                            if !can_edit { return; }
                                                            let v = event_target_value(&ev);
                                                            set_player_inputs.update(|map| {
                                                                if let Some(entry) = map.get_mut(&pid) {
                                                                    entry.1 = v;
                                                                }
                                                            });
                                                        }
                                                    />
                                                    <span class="euro-suffix">"€"</span>
                                                </div>

                                                {can_edit.then(|| view! {
                                                    <button
                                                        type="button"
                                                        class="ghost-btn balance-btn"
                                                        title="Fehlbetrag für Nullsumme (0,00 €) hier eintragen"
                                                        on:click=move |_| auto_balance_player(pid)
                                                    >
                                                        "⚖️ Ausgleichen"
                                                    </button>
                                                })}
                                            </div>
                                        </div>
                                    }
                                }).collect_view()
                            }}
                        </div>

                        // Live Zero-Sum Indicator Bar
                        <div class=move || {
                            if current_sum_cents.get() == 0 {
                                "zero-sum-banner balanced"
                            } else {
                                "zero-sum-banner unbalanced"
                            }
                        }>
                            <div>
                                <strong>"Tisch-Bilanz (Summe): "</strong>
                                <span>{move || format_euros(current_sum_cents.get(), true)}</span>
                            </div>
                            <div class="zero-sum-status">
                                {move || {
                                    let s = current_sum_cents.get();
                                    if s == 0 {
                                        "✅ Nullsumme stimmt exakt (0,00 €)".to_string()
                                    } else {
                                        format!("⚠️ Aktuelle Differenz: {} (Andere Spieler können ihren Wert noch selbst nachtragen)", format_euros(-s, true))
                                    }
                                }}
                            </div>
                        </div>

                        <div class="checkbox-override">
                            <input
                                type="checkbox"
                                id="allow-non-zero"
                                checked=move || allow_non_zero_sum.get()
                                on:change=move |ev| set_allow_non_zero_sum.set(event_target_checked(&ev))
                            />
                            <label for="allow-non-zero">
                                "Speichern erlauben, auch wenn die Summe noch nicht 0,00 € ergibt (z.B. wenn Mitspieler ihren Betrag später selbst eintragen)"
                            </label>
                        </div>
                    </fieldset>

                    {move || error_msg.get().map(|msg| view! {
                        <div class="alert-error" role="alert">{msg}</div>
                    })}

                    <div class="form-actions">
                        <button
                            type="button"
                            class="secondary-btn"
                            on:click=move |_| {
                                set_editing_session_id.set(None);
                                set_active_tab.set(ActiveTab::Dashboard);
                            }
                        >
                            "Abbrechen"
                        </button>
                        <button type="submit" class="primary-btn">
                            "💾 Ergebnis speichern"
                        </button>
                    </div>
                </form>
            </div>
        </section>
    }
}
