mod components;
mod models;
mod storage;

use components::dashboard::Dashboard;
use components::history::HistoryView;
use components::navbar::{ActiveTab, Navbar};
use components::session_form::SessionForm;
use components::settings::SettingsView;
use leptos::prelude::*;
use storage::{
    SupabaseConfig, load_auth_session, load_local_data, pull_from_supabase, save_auth_session,
};
use uuid::Uuid;

#[component]
fn App() -> impl IntoView {
    let initial_config = SupabaseConfig::load();
    let initial_data = load_local_data();
    let initial_auth = load_auth_session(&initial_data);
    let initially_authed = initial_auth.is_authenticated(initial_config.is_configured());

    let initial_status = if !initially_authed {
        "Nicht eingeloggt".to_string()
    } else if initial_config.is_configured() {
        "Synchronisiere…".to_string()
    } else {
        "Lokal gespeichert".to_string()
    };

    let (active_tab, set_active_tab) = signal(if initially_authed {
        ActiveTab::Dashboard
    } else {
        ActiveTab::Settings
    });
    let (config, set_config) = signal(initial_config.clone());
    let (table_data, set_table_data) = signal(initial_data);
    let (auth, set_auth) = signal(initial_auth.clone());
    let (editing_session_id, set_editing_session_id) = signal::<Option<Uuid>>(None);
    let (sync_status, set_sync_status) = signal(initial_status);

    // On startup: only pull from Supabase if both configured AND authenticated
    if initial_config.is_configured() && initially_authed {
        wasm_bindgen_futures::spawn_local(async move {
            match pull_from_supabase(&initial_config, &initial_auth).await {
                Ok(remote) => {
                    if let Some(uid) = initial_auth.user_id {
                        if let Some(p) = remote.players.iter().find(|p| p.claimed_by == Some(uid)) {
                            let mut updated_auth = initial_auth.clone();
                            updated_auth.claimed_player_id = Some(p.id);
                            updated_auth.is_admin = p.is_admin;
                            save_auth_session(&updated_auth);
                            set_auth.set(updated_auth);
                        }
                    }
                    set_table_data.set(remote);
                    set_sync_status.set("Cloud & Lokal synchron".to_string());
                }
                Err(e) => {
                    set_sync_status.set(format!("Offline ({e})"));
                }
            }
        });
    }

    view! {
        <div class="app-shell">
            <Navbar
                active_tab=active_tab
                set_active_tab=set_active_tab
                config=config
                auth=auth
                table_data=table_data
                sync_status=sync_status
            />

            <main class="main-content">
                {move || {
                    let is_authed = auth.get().is_authenticated(config.get().is_configured());
                    if !is_authed {
                        // Unauthenticated users cannot see any dashboard, players, or history data
                        return view! {
                            <div class="auth-gate-banner">
                                "🔒 Bitte melde dich an, um auf das Dashboard, die Spieler und die Schafkopf-Ergebnisse zuzugreifen."
                            </div>
                            <SettingsView
                                table_data=table_data
                                set_table_data=set_table_data
                                config=config
                                set_config=set_config
                                auth=auth
                                set_auth=set_auth
                                set_sync_status=set_sync_status
                            />
                        }.into_any();
                    }

                    match active_tab.get() {
                        ActiveTab::Dashboard => view! {
                            <Dashboard
                                table_data=table_data
                                set_active_tab=set_active_tab
                            />
                        }.into_any(),
                        ActiveTab::LogSession => view! {
                            <SessionForm
                                table_data=table_data
                                set_table_data=set_table_data
                                config=config
                                auth=auth
                                editing_session_id=editing_session_id
                                set_editing_session_id=set_editing_session_id
                                set_sync_status=set_sync_status
                                set_active_tab=set_active_tab
                            />
                        }.into_any(),
                        ActiveTab::History => view! {
                            <HistoryView
                                table_data=table_data
                                set_table_data=set_table_data
                                config=config
                                auth=auth
                                set_editing_session_id=set_editing_session_id
                                set_active_tab=set_active_tab
                                set_sync_status=set_sync_status
                            />
                        }.into_any(),
                        ActiveTab::Settings => view! {
                            <SettingsView
                                table_data=table_data
                                set_table_data=set_table_data
                                config=config
                                set_config=set_config
                                auth=auth
                                set_auth=set_auth
                                set_sync_status=set_sync_status
                            />
                        }.into_any(),
                    }
                }}
            </main>

            <footer class="app-footer">
                <span>"♠ ♥ ♦ ♣ Schafkopf Kasse — Gebaut mit Rust & Leptos (WASM)"</span>
            </footer>
        </div>
    }
}

fn main() {
    mount_to_body(App);
}
