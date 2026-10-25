use crate::models::{AuthSession, TableData};
use crate::storage::SupabaseConfig;
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    LogSession,
    History,
    Settings,
}

#[component]
pub fn Navbar(
    active_tab: ReadSignal<ActiveTab>,
    set_active_tab: WriteSignal<ActiveTab>,
    config: ReadSignal<SupabaseConfig>,
    auth: ReadSignal<AuthSession>,
    table_data: ReadSignal<TableData>,
    sync_status: ReadSignal<String>,
) -> impl IntoView {
    let active_identity_label = Signal::derive(move || {
        let a = auth.get();
        let data = table_data.get();
        if let Some(pid) = a.claimed_player_id {
            if let Some(p) = data.player_by_id(&pid) {
                if a.is_admin {
                    return format!("👑 {} (Admin)", p.name);
                } else {
                    return format!("👤 {}", p.name);
                }
            }
        }
        if a.is_logged_in() {
            "👤 Spieler wählen…".to_string()
        } else {
            "🔒 Nicht angemeldet".to_string()
        }
    });

    let is_authed = Signal::derive(move || auth.get().is_authenticated(config.get().is_configured()));

    view! {
        <header class="top-navbar">
            <div class="brand" on:click=move |_| {
                if is_authed.get() {
                    set_active_tab.set(ActiveTab::Dashboard);
                }
            }>
                <span class="brand-suit">"♠"</span>
                <div class="brand-titles">
                    <h1>"Schafkopf Kasse"</h1>
                    <span class="subtitle">
                        {move || format!("Tisch: {}", config.get().table_slug)}
                    </span>
                </div>
            </div>

            {move || is_authed.get().then(|| view! {
                <nav class="nav-tabs" aria-label="Hauptnavigation">
                    <button
                        type="button"
                        class=move || if active_tab.get() == ActiveTab::Dashboard { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_active_tab.set(ActiveTab::Dashboard)
                    >
                        "📊 Dashboard"
                    </button>
                    <button
                        type="button"
                        class=move || if active_tab.get() == ActiveTab::LogSession { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_active_tab.set(ActiveTab::LogSession)
                    >
                        "➕ Abend eintragen"
                    </button>
                    <button
                        type="button"
                        class=move || if active_tab.get() == ActiveTab::History { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_active_tab.set(ActiveTab::History)
                    >
                        "📜 Verlauf"
                    </button>
                    <button
                        type="button"
                        class=move || if active_tab.get() == ActiveTab::Settings { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_active_tab.set(ActiveTab::Settings)
                    >
                        "🔑 Login & Spieler"
                    </button>
                </nav>
            })}

            <div class="navbar-right">
                <button
                    type="button"
                    class=move || if auth.get().claimed_player_id.is_some() { "identity-pill logged-in" } else { "identity-pill logged-out" }
                    title="Angemeldeter Spieler (Klicken zum Wechseln / Einloggen)"
                    on:click=move |_| set_active_tab.set(ActiveTab::Settings)
                >
                    {move || active_identity_label.get()}
                </button>

                <div class="sync-badge" title="Speicherstatus">
                    <span class=move || {
                        if config.get().is_configured() {
                            "dot cloud"
                        } else {
                            "dot local"
                        }
                    }></span>
                    <span class="sync-text">{move || sync_status.get()}</span>
                </div>
            </div>
        </header>
    }
}
