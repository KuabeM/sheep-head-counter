use crate::models::{AuthSession, DEFAULT_COLORS, Player, TableData, create_demo_data};
use crate::storage::{
    SupabaseConfig, claim_player, clear_auth_session, is_auth_expired_error, pull_from_supabase,
    push_all_local_to_supabase, save_auth_session, save_local_data, supabase_sign_in_password,
    supabase_sign_up, upsert_player_to_supabase,
};
use leptos::prelude::*;
use uuid::Uuid;

#[component]
pub fn SettingsView(
    table_data: ReadSignal<TableData>,
    set_table_data: WriteSignal<TableData>,
    config: ReadSignal<SupabaseConfig>,
    set_config: WriteSignal<SupabaseConfig>,
    auth: ReadSignal<AuthSession>,
    set_auth: WriteSignal<AuthSession>,
    set_sync_status: WriteSignal<String>,
) -> impl IntoView {
    // Login / Signup inputs
    let (auth_email, set_auth_email) = signal(auth.get_untracked().email.unwrap_or_default());
    let (auth_password, set_auth_password) = signal(String::new());

    // Player management inputs (Admin only)
    let (new_player_name, set_new_player_name) = signal(String::new());
    let (new_player_color, set_new_player_color) = signal(DEFAULT_COLORS[4].to_string());

    // Supabase config inputs
    let init_cfg = config.get_untracked();
    let (url_input, set_url_input) = signal(init_cfg.project_url);
    let (key_input, set_key_input) = signal(init_cfg.publishable_key);
    let (slug_input, set_slug_input) = signal(init_cfg.table_slug);
    let (feedback_msg, set_feedback_msg) = signal::<Option<(bool, String)>>(None);

    // JSON Import/Export state
    let (json_box, set_json_box) = signal(String::new());

    // 1. Login via Supabase Auth (or local email session if offline)
    let on_login = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let email = auth_email.get().trim().to_string();
        let password = auth_password.get();
        let cfg = config.get();

        if email.is_empty() {
            set_feedback_msg.set(Some((false, "Bitte eine E-Mail-Adresse eingeben.".to_string())));
            return;
        }

        if cfg.is_configured() {
            set_sync_status.set("Anmeldung läuft…".to_string());
            wasm_bindgen_futures::spawn_local(async move {
                match supabase_sign_in_password(&cfg, &email, &password).await {
                    Ok(mut session) => {
                        if let Ok(remote_data) = pull_from_supabase(&cfg, &session).await {
                            // Auto-match claimed player if already claimed in DB
                            if let Some(uid) = session.user_id {
                                if let Some(p) = remote_data.players.iter().find(|p| p.claimed_by == Some(uid)) {
                                    session.claimed_player_id = Some(p.id);
                                    session.is_admin = p.is_admin;
                                    save_auth_session(&session);
                                }
                            }
                            set_table_data.set(remote_data);
                        }
                        set_auth.set(session);
                        set_sync_status.set("Angemeldet (Cloud)".to_string());
                        set_feedback_msg.set(Some((
                            true,
                            "Erfolgreich eingeloggt! Bitte wähle unten dein Spielerprofil aus.".to_string(),
                        )));
                    }
                    Err(e) => {
                        set_sync_status.set("Login-Fehler".to_string());
                        set_feedback_msg.set(Some((false, e)));
                    }
                }
            });
        } else {
            // Offline / Local mode login
            let uid = Uuid::new_v4();
            let session = AuthSession {
                user_id: Some(uid),
                email: Some(email.clone()),
                access_token: None,
                claimed_player_id: None,
                is_admin: false,
            };
            save_auth_session(&session);
            set_auth.set(session);
            set_feedback_msg.set(Some((
                true,
                format!("Lokal angemeldet als {email}. Wähle jetzt rechts dein Spielerprofil aus!"),
            )));
        }
    };

    let on_register = move |_| {
        let email = auth_email.get().trim().to_string();
        let password = auth_password.get();
        let cfg = config.get();

        if !cfg.is_configured() {
            set_feedback_msg.set(Some((
                false,
                "Für die Cloud-Registrierung muss unten zuerst die Supabase URL & der Publishable Key hinterlegt sein. Im lokalen Modus reicht ein Klick auf 'Einloggen'.".to_string(),
            )));
            return;
        }

        wasm_bindgen_futures::spawn_local(async move {
            match supabase_sign_up(&cfg, &email, &password).await {
                Ok(session) => {
                    set_auth.set(session);
                    set_feedback_msg.set(Some((
                        true,
                        "Account registriert! Wähle jetzt dein Spielerprofil aus der Liste.".to_string(),
                    )));
                }
                Err(e) => {
                    if is_auth_expired_error(&e) {
                        set_auth.set(AuthSession::default());
                        set_sync_status.set("Sitzung abgelaufen".to_string());
                    }
                    set_feedback_msg.set(Some((false, e)));
                }
            }
        });
    };

    let on_logout = move |_| {
        clear_auth_session();
        set_auth.set(AuthSession::default());
        set_feedback_msg.set(Some((true, "Abgemeldet.".to_string())));
    };

    // 2. Claim one of the configured players
    let on_select_player = move |target_pid: Uuid| {
        let cfg = config.get();
        let cur_auth = auth.get();
        let mut data = table_data.get();

        wasm_bindgen_futures::spawn_local(async move {
            match claim_player(&cfg, &cur_auth, &mut data, target_pid).await {
                Ok(new_auth) => {
                    let p_name = data
                        .player_by_id(&target_pid)
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    let role_note = if new_auth.is_admin {
                        "👑 Du hast Admin-Rechte (kannst alle Spieler & Abende verwalten)."
                    } else {
                        "🔒 Du kannst ab sofort deine eigenen Ergebnisse eintragen und bearbeiten."
                    };
                    set_table_data.set(data);
                    set_auth.set(new_auth);
                    set_feedback_msg.set(Some((
                        true,
                        format!("Spielerprofil '{p_name}' verknüpft! {role_note}"),
                    )));
                }
                Err(e) => {
                    if is_auth_expired_error(&e) {
                        set_auth.set(AuthSession::default());
                        set_sync_status.set("Sitzung abgelaufen".to_string());
                    }
                    set_feedback_msg.set(Some((false, e)));
                }
            }
        });
    };

    // 3. Admin: Toggle Admin status for any player
    let toggle_player_admin = move |pid: Uuid| {
        let cur_auth = auth.get();
        if !cur_auth.can_manage_table() {
            return;
        }
        let mut data = table_data.get();
        let mut updated_player = None;
        if let Some(p) = data.players.iter_mut().find(|p| p.id == pid) {
            p.is_admin = !p.is_admin;
            updated_player = Some(p.clone());
        }
        save_local_data(&data);
        set_table_data.set(data);

        // If toggling own admin status, update auth session
        if cur_auth.claimed_player_id == Some(pid) {
            if let Some(ref up) = updated_player {
                let mut a = cur_auth.clone();
                a.is_admin = up.is_admin;
                save_auth_session(&a);
                set_auth.set(a);
            }
        }

        if let Some(up) = updated_player {
            let cfg = config.get();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = upsert_player_to_supabase(&cfg, &cur_auth, &up).await;
            });
        }
    };

    // 4. Admin: Add new player to table roster
    let on_add_player = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let cur_auth = auth.get();
        if !cur_auth.can_manage_table() {
            set_feedback_msg.set(Some((
                false,
                "Nur der Admin kann neue Spieler anlegen.".to_string(),
            )));
            return;
        }
        let name = new_player_name.get().trim().to_string();
        if name.is_empty() {
            return;
        }
        let mut data = table_data.get();
        let new_p = Player {
            id: Uuid::new_v4(),
            name,
            avatar_color: new_player_color.get(),
            active: true,
            claimed_by: None,
            is_admin: false,
        };
        data.players.push(new_p.clone());
        save_local_data(&data);
        set_table_data.set(data);
        set_new_player_name.set(String::new());

        let cfg = config.get();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = upsert_player_to_supabase(&cfg, &cur_auth, &new_p).await;
        });
    };

    let remove_or_toggle_player = move |pid: Uuid| {
        if !auth.get().can_manage_table() {
            return;
        }
        let mut data = table_data.get();
        let has_sessions = data
            .sessions
            .iter()
            .any(|s| s.results.iter().any(|r| r.player_id == pid));
        if has_sessions {
            if let Some(p) = data.players.iter_mut().find(|p| p.id == pid) {
                p.active = !p.active;
            }
        } else {
            data.players.retain(|p| p.id != pid);
        }
        save_local_data(&data);
        set_table_data.set(data);
    };

    let on_save_supabase = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let new_cfg = SupabaseConfig {
            project_url: url_input.get().trim().to_string(),
            publishable_key: key_input.get().trim().to_string(),
            table_slug: slug_input.get().trim().to_lowercase(),
        };
        new_cfg.save();
        set_config.set(new_cfg.clone());

        if !new_cfg.is_configured() {
            set_sync_status.set("Lokal gespeichert".to_string());
            set_feedback_msg.set(Some((
                true,
                "Supabase-Verbindung deaktiviert (Lokaler Modus aktiv).".to_string(),
            )));
            return;
        }

        let cur_auth = auth.get();
        if !cur_auth.is_authenticated(true) {
            set_sync_status.set("Bitte einloggen".to_string());
            set_feedback_msg.set(Some((
                true,
                "Supabase URL & Key gespeichert! Bitte logge dich jetzt oben unter '1. Login' ein, um die Daten zu laden.".to_string(),
            )));
            return;
        }

        set_sync_status.set("Verbinde mit Supabase…".to_string());
        wasm_bindgen_futures::spawn_local(async move {
            match pull_from_supabase(&new_cfg, &cur_auth).await {
                Ok(remote_data) => {
                    // Refresh claimed player and admin status from remote players list
                    if let Some(uid) = cur_auth.user_id {
                        if let Some(p) = remote_data.players.iter().find(|p| p.claimed_by == Some(uid)) {
                            let mut updated_auth = cur_auth.clone();
                            updated_auth.claimed_player_id = Some(p.id);
                            updated_auth.is_admin = p.is_admin;
                            save_auth_session(&updated_auth);
                            set_auth.set(updated_auth);
                        }
                    }
                    let count = remote_data.sessions.len();
                    set_table_data.set(remote_data);
                    set_sync_status.set("Cloud & Lokal synchron".to_string());
                    set_feedback_msg.set(Some((
                        true,
                        format!("Erfolgreich mit Supabase verbunden ({count} Spielabende aus Cloud geladen)!"),
                    )));
                }
                Err(e) => {
                    set_sync_status.set("Sync-Fehler".to_string());
                    set_feedback_msg.set(Some((false, e)));
                }
            }
        });
    };

    let on_push_all_cloud = move |_| {
        let cfg = config.get();
        let cur_auth = auth.get();
        let data = table_data.get();
        set_sync_status.set("Lade lokalen Stand hoch…".to_string());
        wasm_bindgen_futures::spawn_local(async move {
            match push_all_local_to_supabase(&cfg, &cur_auth, &data).await {
                Ok(()) => {
                    set_sync_status.set("Cloud & Lokal synchron".to_string());
                    set_feedback_msg.set(Some((
                        true,
                        "Alle lokalen Spieler & Spielabende wurden erfolgreich in Supabase gespeichert!".to_string(),
                    )));
                }
                Err(e) => {
                    set_sync_status.set("Sync-Fehler".to_string());
                    set_feedback_msg.set(Some((false, e)));
                }
            }
        });
    };

    let export_json = move |_| {
        if let Ok(pretty) = serde_json::to_string_pretty(&table_data.get()) {
            set_json_box.set(pretty);
            set_feedback_msg.set(Some((
                true,
                "JSON-Backup unten im Textfeld generiert.".to_string(),
            )));
        }
    };

    let import_json = move |_| {
        if !auth.get().can_manage_table() {
            set_feedback_msg.set(Some((
                false,
                "Nur der Admin darf ein komplettes JSON-Backup importieren.".to_string(),
            )));
            return;
        }
        let raw = json_box.get();
        match serde_json::from_str::<TableData>(&raw) {
            Ok(parsed) => {
                save_local_data(&parsed);
                set_table_data.set(parsed);
                set_feedback_msg.set(Some((
                    true,
                    "JSON-Daten erfolgreich importiert!".to_string(),
                )));
            }
            Err(e) => {
                set_feedback_msg.set(Some((false, format!("Ungültiges JSON-Format: {e}"))));
            }
        }
    };

    let load_demo = move |_| {
        let demo = create_demo_data();
        save_local_data(&demo);
        set_table_data.set(demo);
        set_feedback_msg.set(Some((
            true,
            "Beispiel-Stammtisch geladen. Wähle jetzt dein Spielerprofil aus!".to_string(),
        )));
    };

    view! {
        <section class="settings-grid">
            {move || feedback_msg.get().map(|(ok, msg)| view! {
                <div class={if ok { "alert-success full-span" } else { "alert-error full-span" }} role="status">
                    {msg}
                </div>
            })}

            // 1. Account Login (hidden when logged in) & Player Identity Selection
            <div class="card">
                <div class="card-header">
                    <h2>
                        {move || if auth.get().is_authenticated(config.get().is_configured()) {
                            "👤 1. Deine Spieler-Identität"
                        } else {
                            "🔑 Login"
                        }}
                    </h2>
                    <span class="card-subtitle">
                        {move || if auth.get().is_authenticated(config.get().is_configured()) {
                            let email_str = auth.get().email.unwrap_or_else(|| "Aktiv".to_string());
                            format!("Angemeldet als {email_str}. Wähle deinen Namen aus der Runde – jeder Spieler kann nur seine eigenen Ergebnisse bearbeiten.")
                        } else {
                            "Bitte melde dich mit deiner E-Mail und deinem Passwort an.".to_string()
                        }}
                    </span>
                </div>

                {move || (!auth.get().is_authenticated(config.get().is_configured())).then(|| view! {
                    <form on:submit=on_login class="session-form" style="margin-bottom: 1.25rem;">
                        <div class="form-row-2">
                            <div class="form-group">
                                <label for="auth-email">"E-Mail-Adresse"</label>
                                <input
                                    type="text"
                                    id="auth-email"
                                    name="email"
                                    autocomplete="email"
                                    placeholder="korbinian@example.com"
                                    required
                                    prop:value=move || auth_email.get()
                                    on:input=move |ev| set_auth_email.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-group">
                                <label for="auth-password">"Passwort"</label>
                                <input
                                    type="password"
                                    id="auth-password"
                                    name="password"
                                    autocomplete="current-password"
                                    placeholder="••••••••"
                                    prop:value=move || auth_password.get()
                                    on:input=move |ev| set_auth_password.set(event_target_value(&ev))
                                />
                            </div>
                        </div>

                        <div class="form-actions">
                            <button type="button" class="secondary-btn" on:click=on_register>
                                "Registrieren"
                            </button>
                            <button type="submit" class="primary-btn">
                                "🔑 Einloggen"
                            </button>
                        </div>
                    </form>
                })}

                {move || auth.get().is_authenticated(config.get().is_configured()).then(|| view! {
                    <div class="claim-box" style="border-top: none; padding-top: 0;">
                        <h3 class="sub-heading">"Wer bist du am Tisch? (Profil auswählen)"</h3>
                        <div class="claim-grid">
                            {move || {
                                let cur_auth = auth.get();
                                table_data.get().players.into_iter().map(|p| {
                                    let pid = p.id;
                                    let is_mine = cur_auth.claimed_player_id == Some(pid);
                                    let taken_by_other = p.claimed_by.is_some()
                                        && p.claimed_by != cur_auth.user_id
                                        && !cur_auth.is_admin;

                                    view! {
                                        <button
                                            type="button"
                                            disabled=taken_by_other
                                            class={if is_mine {
                                                "claim-card selected"
                                            } else if taken_by_other {
                                                "claim-card disabled"
                                            } else {
                                                "claim-card"
                                            }}
                                            on:click=move |_| on_select_player(pid)
                                        >
                                            <div class="claim-card-top">
                                                <span class="color-dot" style=format!("background-color: {};", p.avatar_color)></span>
                                                <strong>{p.name}</strong>
                                                {p.is_admin.then(|| view! { <span class="badge-admin">"👑 Admin"</span> })}
                                            </div>
                                            <span class="claim-status">
                                                {if is_mine {
                                                    "✅ Dein aktives Profil"
                                                } else if taken_by_other {
                                                    "🔒 Bereits vergeben"
                                                } else {
                                                    "👉 Als mein Profil wählen"
                                                }}
                                            </span>
                                        </button>
                                    }
                                }).collect_view()
                            }}
                        </div>

                        <div class="form-actions" style="margin-top: 1.25rem;">
                            <button type="button" class="danger-ghost-btn" on:click=on_logout>
                                "🚪 Abmelden"
                            </button>
                        </div>
                    </div>
                })}
            </div>

            // 2. Player Roster Management (Only visible when logged in)
            {move || auth.get().is_authenticated(config.get().is_configured()).then(|| view! {
                <div class="card">
                    <div class="card-header">
                        <h2>"👥 2. Konfigurierte Spieler (Stammtisch)"</h2>
                        <span class="card-subtitle">
                            {move || if auth.get().can_manage_table() {
                                "👑 Admin-Modus aktiv: Du kannst Spieler hinzufügen, entfernen und Admin-Rechte vergeben."
                            } else {
                                "🔒 Nur der Admin (Korbinian) kann die Spielerliste bearbeiten."
                            }}
                        </span>
                    </div>

                    <ul class="player-roster">
                        {move || {
                            let is_adm = auth.get().can_manage_table();
                            table_data.get().players.into_iter().map(|p| {
                                let pid = p.id;
                                let p_is_admin = p.is_admin;
                                view! {
                                    <li class="roster-item">
                                        <div class="roster-info">
                                            <span class="color-dot" style=format!("background-color: {};", p.avatar_color)></span>
                                            <strong>{p.name}</strong>
                                            {p_is_admin.then(|| view! { <span class="badge-admin">"👑 Admin"</span> })}
                                            {(!p.active).then(|| view! { <span class="badge-muted">"pausiert"</span> })}
                                        </div>
                                        {is_adm.then(|| view! {
                                            <div class="roster-actions">
                                                <button
                                                    type="button"
                                                    class="ghost-btn"
                                                    title="Admin-Rolle umschalten"
                                                    on:click=move |_| toggle_player_admin(pid)
                                                >
                                                    {if p_is_admin { "Admin entziehen" } else { "Zum Admin machen" }}
                                                </button>
                                                <button
                                                    type="button"
                                                    class="ghost-btn"
                                                    on:click=move |_| remove_or_toggle_player(pid)
                                                >
                                                    {if p.active { "Pausieren/Löschen" } else { "Aktivieren" }}
                                                </button>
                                            </div>
                                        })}
                                    </li>
                                }
                            }).collect_view()
                        }}
                    </ul>

                    {move || auth.get().can_manage_table().then(|| view! {
                        <form on:submit=on_add_player class="inline-add-form">
                            <div class="form-group grow">
                                <label for="new-player-name">"Neuen Spieler zur Runde hinzufügen"</label>
                                <input
                                    type="text"
                                    id="new-player-name"
                                    placeholder="z.B. Franz"
                                    required
                                    prop:value=move || new_player_name.get()
                                    on:input=move |ev| set_new_player_name.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-group">
                                <label for="new-player-color">"Farbe"</label>
                                <input
                                    type="color"
                                    id="new-player-color"
                                    prop:value=move || new_player_color.get()
                                    on:input=move |ev| set_new_player_color.set(event_target_value(&ev))
                                />
                            </div>
                            <button type="submit" class="primary-btn">"➕ Hinzufügen"</button>
                        </form>
                    })}
                </div>
            })}

            // 3. Supabase Cloud-Verbindung (Strictly visible to Admin only)
            {move || auth.get().can_manage_table().then(|| view! {
                <div class="card">
                    <div class="card-header">
                        <h2>"☁️ Supabase Projekt-Verbindung (Admin)"</h2>
                        <span class="card-subtitle">
                            "Verbindet die App mit deiner kostenlosen Supabase Postgres-Datenbank inkl. Row-Level Security (RLS)."
                        </span>
                    </div>

                    <form on:submit=on_save_supabase class="session-form">
                        <div class="form-group">
                            <label for="sb-url">"Supabase Project URL"</label>
                            <input
                                type="url"
                                id="sb-url"
                                placeholder="https://xyzcompany.supabase.co"
                                prop:value=move || url_input.get()
                                on:input=move |ev| set_url_input.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="form-group">
                            <label for="sb-key">"Supabase Publishable Key"</label>
                            <input
                                type="password"
                                id="sb-key"
                                placeholder="sb_publishable_..."
                                prop:value=move || key_input.get()
                                on:input=move |ev| set_key_input.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="form-group">
                            <label for="sb-slug">"Anzeigename des Tisches"</label>
                            <input
                                type="text"
                                id="sb-slug"
                                placeholder="stammtisch"
                                required
                                prop:value=move || slug_input.get()
                                on:input=move |ev| set_slug_input.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="form-actions">
                            <button type="button" class="secondary-btn" on:click=on_push_all_cloud>
                                "⬆️ Lokalen Stand in Cloud hochladen (Admin)"
                            </button>
                            <button type="submit" class="primary-btn">
                                "🔄 Speichern & Synchronisieren"
                            </button>
                        </div>
                    </form>
                </div>
            })}

            // 4. Backup & Demo Data (Only visible when logged in)
            {move || auth.get().is_authenticated(config.get().is_configured()).then(|| view! {
                <div class="card">
                    <div class="card-header">
                        <h2>"📦 4. JSON Backup & Demo-Daten"</h2>
                        <span class="card-subtitle">
                            "Exportiere jederzeit den gesamten Datenbestand als JSON."
                        </span>
                    </div>

                    <div class="backup-actions">
                        <button type="button" class="secondary-btn" on:click=export_json>
                            "📤 JSON Exportieren"
                        </button>
                        {move || auth.get().can_manage_table().then(|| view! {
                            <button type="button" class="secondary-btn" on:click=import_json>
                                "📥 JSON Importieren (Admin)"
                            </button>
                        })}
                        <button type="button" class="ghost-btn" on:click=load_demo>
                            "🎲 Beispiel-Daten laden"
                        </button>
                    </div>

                    <div class="form-group">
                        <label for="json-backup-box">"JSON-Daten"</label>
                        <textarea
                            id="json-backup-box"
                            rows="5"
                            placeholder="Klicke auf 'JSON Exportieren'…"
                            prop:value=move || json_box.get()
                            on:input=move |ev| set_json_box.set(event_target_value(&ev))
                        ></textarea>
                    </div>
                </div>
            })}
        </section>
    }
}
