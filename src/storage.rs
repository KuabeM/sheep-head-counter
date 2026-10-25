use crate::models::{AuthSession, GameSession, Player, SessionResult, TableData, create_demo_data};
use gloo_net::http::{Request, RequestBuilder};
use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const STORAGE_KEY_DATA: &str = "schafkopf_table_data_v2";
const STORAGE_KEY_CONFIG: &str = "schafkopf_supabase_config_v2";
const STORAGE_KEY_AUTH: &str = "schafkopf_auth_session_v2";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupabaseConfig {
    pub project_url: String,
    #[serde(alias = "anon_key")]
    pub publishable_key: String,
    pub table_slug: String,
}

impl SupabaseConfig {
    pub fn load() -> Self {
        let mut cfg: Self = LocalStorage::get(STORAGE_KEY_CONFIG)
            .or_else(|_| LocalStorage::get("schafkopf_supabase_config_v1"))
            .unwrap_or_default();
        if cfg.project_url.is_empty() {
            if let Some(env_url) = option_env!("SUPABASE_URL") {
                cfg.project_url = env_url.to_string();
            }
        }
        if cfg.publishable_key.is_empty() {
            if let Some(env_key) = option_env!("SUPABASE_PUBLISHABLE_KEY")
                .or(option_env!("SUPABASE_ANON_KEY"))
            {
                cfg.publishable_key = env_key.to_string();
            }
        }
        if cfg.table_slug.is_empty() {
            cfg.table_slug = "stammtisch".to_string();
        }
        cfg
    }

    pub fn save(&self) {
        let _ = LocalStorage::set(STORAGE_KEY_CONFIG, self);
    }

    pub fn is_configured(&self) -> bool {
        !self.project_url.trim().is_empty() && !self.publishable_key.trim().is_empty()
    }
}

pub fn load_local_data() -> TableData {
    if let Ok(data) = LocalStorage::get::<TableData>(STORAGE_KEY_DATA) {
        data
    } else if let Ok(legacy) = LocalStorage::get::<TableData>("schafkopf_table_data_v1") {
        save_local_data(&legacy);
        legacy
    } else {
        let demo = create_demo_data();
        save_local_data(&demo);
        demo
    }
}

pub fn save_local_data(data: &TableData) {
    let _ = LocalStorage::set(STORAGE_KEY_DATA, data);
}

pub fn load_auth_session(data: &TableData) -> AuthSession {
    if let Ok(mut session) = LocalStorage::get::<AuthSession>(STORAGE_KEY_AUTH) {
        if let Some(uid) = session.user_id {
            if let Some(p) = data.players.iter().find(|p| p.claimed_by == Some(uid)) {
                session.claimed_player_id = Some(p.id);
                session.is_admin = p.is_admin;
            } else if let Some(pid) = session.claimed_player_id {
                if let Some(p) = data.players.iter().find(|p| p.id == pid) {
                    session.is_admin = p.is_admin;
                }
            }
        }
        session
    } else {
        AuthSession::default()
    }
}

pub fn save_auth_session(session: &AuthSession) {
    let _ = LocalStorage::set(STORAGE_KEY_AUTH, session);
}

pub fn clear_auth_session() {
    LocalStorage::delete(STORAGE_KEY_AUTH);
}

/// Attaches the Supabase `apikey` and `Authorization: Bearer` headers.
/// Supabase API Gateway translates `sb_publishable_...` keys when sent in `apikey` and `Authorization`,
/// or uses the logged-in user's JWT `access_token` when authenticated.
fn apply_supabase_headers(
    req: RequestBuilder,
    config: &SupabaseConfig,
    auth: Option<&AuthSession>,
) -> RequestBuilder {
    let key = config.publishable_key.trim();
    let req = req.header("apikey", key);

    if let Some(tok) = auth.and_then(|a| a.access_token.as_deref()) {
        if !tok.trim().is_empty() {
            return req.header("Authorization", &format!("Bearer {}", tok.trim()));
        }
    }

    req.header("Authorization", &format!("Bearer {key}"))
}

// =============================================================================
// Supabase GoTrue Auth (Email + Password & Signup)
// =============================================================================

#[derive(Serialize)]
struct EmailPasswordPayload<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct GoTrueUser {
    id: Uuid,
    email: Option<String>,
}

#[derive(Deserialize)]
struct GoTrueTokenResponse {
    access_token: Option<String>,
    user: Option<GoTrueUser>,
}

#[derive(Deserialize)]
struct GoTrueSignupResponse {
    access_token: Option<String>,
    user: Option<GoTrueUser>,
    // When email confirmation is enabled, `/auth/v1/signup` returns the user fields at the top level
    id: Option<Uuid>,
    email: Option<String>,
}

/// Logs in with Email + Password via Supabase Auth (`/auth/v1/token?grant_type=password`)
pub async fn supabase_sign_in_password(
    config: &SupabaseConfig,
    email: &str,
    password: &str,
) -> Result<AuthSession, String> {
    if !config.is_configured() {
        return Err("Supabase URL & Publishable Key sind noch nicht konfiguriert.".to_string());
    }
    let base = config.project_url.trim_end_matches('/');
    let url = format!("{base}/auth/v1/token?grant_type=password");

    let req = apply_supabase_headers(Request::post(&url), config, None)
        .header("Content-Type", "application/json")
        .json(&EmailPasswordPayload { email, password })
        .map_err(|e| e.to_string())?;

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Netzwerkfehler: {e}"))?;

    if !resp.ok() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("Login fehlgeschlagen ({}): {txt}", resp.status()));
    }

    let parsed: GoTrueTokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Ungültige Auth-Antwort: {e}"))?;

    let user = parsed
        .user
        .ok_or_else(|| "Kein User-Objekt in Antwort erhalten.".to_string())?;

    let session = AuthSession {
        user_id: Some(user.id),
        email: user.email,
        access_token: parsed.access_token,
        claimed_player_id: None,
        is_admin: false,
    };
    save_auth_session(&session);
    Ok(session)
}

/// Registers a new account with Email + Password via Supabase Auth (`/auth/v1/signup`)
pub async fn supabase_sign_up(
    config: &SupabaseConfig,
    email: &str,
    password: &str,
) -> Result<AuthSession, String> {
    if !config.is_configured() {
        return Err("Supabase URL & Publishable Key sind noch nicht konfiguriert.".to_string());
    }
    let base = config.project_url.trim_end_matches('/');
    let url = format!("{base}/auth/v1/signup");

    let req = apply_supabase_headers(Request::post(&url), config, None)
        .header("Content-Type", "application/json")
        .json(&EmailPasswordPayload { email, password })
        .map_err(|e| e.to_string())?;

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Netzwerkfehler: {e}"))?;

    if !resp.ok() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Registrierung fehlgeschlagen ({}): {txt}",
            resp.status()
        ));
    }

    let parsed: GoTrueSignupResponse = resp
        .json()
        .await
        .map_err(|e| format!("Ungültige Auth-Antwort: {e}"))?;

    if let Some(user) = parsed.user {
        let session = AuthSession {
            user_id: Some(user.id),
            email: user.email,
            access_token: parsed.access_token,
            claimed_player_id: None,
            is_admin: false,
        };
        save_auth_session(&session);
        Ok(session)
    } else if parsed.id.is_some() {
        let _ = parsed.email;
        Err("Registrierung erfolgreich! Bitte bestätige den Link in deiner E-Mail (oder deaktiviere 'Confirm email' in Supabase onder Authentication → Providers → Email) und klicke dann auf 'Einloggen'.".to_string())
    } else {
        Err("Unerwartete Antwort von Supabase Auth.".to_string())
    }
}

/// Claims one of the configured players for the currently logged-in user
pub async fn claim_player(
    config: &SupabaseConfig,
    auth: &AuthSession,
    data: &mut TableData,
    target_player_id: Uuid,
) -> Result<AuthSession, String> {
    let mut updated_auth = auth.clone();

    if config.is_configured() {
        let Some(ref tok) = auth.access_token else {
            return Err("Du bist noch nicht bei Supabase eingeloggt (kein Access-Token). Bitte gib oben E-Mail & Passwort ein und klicke auf '🔑 Einloggen' (oder 'Registrieren').".to_string());
        };
        if tok.trim().is_empty() {
            return Err("Kein gültiges Supabase-Login-Token vorhanden. Bitte zuerst mit E-Mail & Passwort einloggen.".to_string());
        }

        let base = config.project_url.trim_end_matches('/');
        let url = format!("{base}/rest/v1/rpc/claim_player_profile");

        #[derive(Serialize)]
        struct ClaimArgs {
            p_player_id: Uuid,
        }

        let req = apply_supabase_headers(Request::post(&url), config, Some(auth))
            .header("Content-Type", "application/json")
            .json(&ClaimArgs {
                p_player_id: target_player_id,
            })
            .map_err(|e| e.to_string())?;

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Netzwerkfehler beim Spieler-Verknüpfen: {e}"))?;

        if !resp.ok() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("Profil konnte nicht gewählt werden ({}): {txt}", resp.status()));
        }

        let claimed_player: Player = resp
            .json()
            .await
            .map_err(|e| format!("Antwort-Fehler: {e}"))?;

        for p in &mut data.players {
            if p.claimed_by == updated_auth.user_id {
                p.claimed_by = None;
            }
            if p.id == claimed_player.id {
                *p = claimed_player.clone();
            }
        }
        updated_auth.claimed_player_id = Some(claimed_player.id);
        updated_auth.is_admin = claimed_player.is_admin;
    } else {
        // Offline / Local mode: bind the chosen player directly
        let uid = updated_auth.user_id.unwrap_or_else(Uuid::new_v4);
        updated_auth.user_id = Some(uid);

        let mut is_adm = false;
        for p in &mut data.players {
            if p.id == target_player_id {
                p.claimed_by = Some(uid);
                is_adm = p.is_admin;
            }
        }
        updated_auth.claimed_player_id = Some(target_player_id);
        updated_auth.is_admin = is_adm;
    }

    save_local_data(data);
    save_auth_session(&updated_auth);
    Ok(updated_auth)
}

// =============================================================================
// Normalized PostgREST Sync (players, sessions, session_results)
// =============================================================================

#[derive(Deserialize)]
struct DbSessionRow {
    id: Uuid,
    played_at: String,
    duration_minutes: Option<u32>,
    #[serde(default)]
    note: String,
    #[serde(default)]
    session_results: Vec<SessionResult>,
}

/// Pulls players, sessions, and session_results from Supabase PostgREST
pub async fn pull_from_supabase(
    config: &SupabaseConfig,
    auth: &AuthSession,
) -> Result<TableData, String> {
    if !config.is_configured() {
        return Err("Supabase ist nicht konfiguriert.".to_string());
    }
    if !auth.is_authenticated(true) {
        return Err("Bitte zuerst einloggen, um Daten zu laden.".to_string());
    }

    let base = config.project_url.trim_end_matches('/');

    // 1. Fetch players
    let players_url = format!("{base}/rest/v1/players?select=*&order=created_at.asc");
    let p_resp = apply_supabase_headers(Request::get(&players_url), config, Some(auth))
        .send()
        .await
        .map_err(|e| format!("Fehler beim Laden der Spieler: {e}"))?;

    if !p_resp.ok() {
        let status = p_resp.status();
        let txt = p_resp.text().await.unwrap_or_default();
        if status == 404 || txt.contains("PGRST205") || txt.contains("players") {
            return Err(format!(
                "Tabelle 'public.players' nicht in Supabase gefunden ({status}). Bitte führe die Datei 'supabase/schema.sql' im Supabase SQL Editor aus! Details: {txt}"
            ));
        }
        return Err(format!("Supabase Fehler ({status}): {txt}"));
    }
    let players: Vec<Player> = p_resp
        .json()
        .await
        .map_err(|e| format!("Spieler-JSON Fehler: {e}"))?;

    // 2. Fetch sessions with embedded session_results
    let sessions_url = format!(
        "{base}/rest/v1/sessions?select=id,played_at,duration_minutes,note,session_results(player_id,amount_cents)&order=played_at.asc"
    );
    let s_resp = apply_supabase_headers(Request::get(&sessions_url), config, Some(auth))
        .send()
        .await
        .map_err(|e| format!("Fehler beim Laden der Spielabende: {e}"))?;

    if !s_resp.ok() {
        let txt = s_resp.text().await.unwrap_or_default();
        return Err(format!("Supabase Fehler ({}): {txt}", s_resp.status()));
    }
    let rows: Vec<DbSessionRow> = s_resp
        .json()
        .await
        .map_err(|e| format!("Sessions-JSON Fehler: {e}"))?;

    let sessions = rows
        .into_iter()
        .map(|r| GameSession {
            id: r.id,
            played_at: r.played_at,
            duration_minutes: r.duration_minutes,
            note: r.note,
            results: r.session_results,
        })
        .collect();

    let table_data = TableData {
        table_slug: config.table_slug.clone(),
        players,
        sessions,
    };
    save_local_data(&table_data);
    Ok(table_data)
}

#[derive(Serialize)]
struct UpsertSessionRow<'a> {
    id: Uuid,
    played_at: &'a str,
    duration_minutes: Option<u32>,
    note: &'a str,
}

#[derive(Serialize)]
struct UpsertResultRow {
    session_id: Uuid,
    player_id: Uuid,
    amount_cents: i64,
}

/// Saves a session and only the permitted player results to Supabase (enforced both here and by Postgres RLS)
pub async fn save_session_to_supabase(
    config: &SupabaseConfig,
    auth: &AuthSession,
    session: &GameSession,
) -> Result<(), String> {
    if !config.is_configured() {
        return Ok(());
    }

    if auth.access_token.as_deref().unwrap_or_default().is_empty() {
        return Err("Nicht bei Supabase eingeloggt (kein Access-Token). Bitte unter '🔑 Login & Spieler' mit E-Mail & Passwort einloggen!".to_string());
    }

    let base = config.project_url.trim_end_matches('/');

    // 1. Upsert session metadata
    let s_url = format!("{base}/rest/v1/sessions?on_conflict=id");
    let s_row = UpsertSessionRow {
        id: session.id,
        played_at: &session.played_at,
        duration_minutes: session.duration_minutes,
        note: &session.note,
    };

    let s_resp = apply_supabase_headers(Request::post(&s_url), config, Some(auth))
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates")
        .json(&s_row)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| format!("Netzwerkfehler: {e}"))?;

    if !s_resp.ok() {
        let txt = s_resp.text().await.unwrap_or_default();
        return Err(format!(
            "Abend konnte nicht gespeichert werden ({}): {txt}",
            s_resp.status()
        ));
    }

    // 2. Filter results to only those the logged-in user is allowed to edit (own player_id, or all if Admin)
    let allowed_rows: Vec<UpsertResultRow> = session
        .results
        .iter()
        .filter(|r| auth.can_edit_player(&r.player_id))
        .map(|r| UpsertResultRow {
            session_id: session.id,
            player_id: r.player_id,
            amount_cents: r.amount_cents,
        })
        .collect();

    if allowed_rows.is_empty() {
        return Ok(());
    }

    let r_url = format!("{base}/rest/v1/session_results?on_conflict=session_id,player_id");
    let r_resp = apply_supabase_headers(Request::post(&r_url), config, Some(auth))
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates")
        .json(&allowed_rows)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| format!("Netzwerkfehler: {e}"))?;

    if !r_resp.ok() {
        let txt = r_resp.text().await.unwrap_or_default();
        return Err(format!(
            "Spieler-Ergebnis abgelehnt (RLS Schutz {}): {txt}",
            r_resp.status()
        ));
    }

    Ok(())
}

/// Pushes all local players and sessions to Supabase (Admin bulk sync / migration helper)
pub async fn push_all_local_to_supabase(
    config: &SupabaseConfig,
    auth: &AuthSession,
    data: &TableData,
) -> Result<(), String> {
    if !config.is_configured() {
        return Err("Supabase URL & Publishable Key sind noch nicht konfiguriert.".to_string());
    }
    if auth.access_token.as_deref().unwrap_or_default().is_empty() {
        return Err("Bitte zuerst oben unter '1. Login' mit deinem Supabase-Account einloggen und dein Admin-Profil ('Korbinian') auswählen!".to_string());
    }
    if !auth.can_manage_table() {
        return Err("Nur der Admin (Korbinian) kann den kompletten lokalen Stand hochladen.".to_string());
    }

    for p in &data.players {
        upsert_player_to_supabase(config, auth, p).await?;
    }
    for s in &data.sessions {
        save_session_to_supabase(config, auth, s).await?;
    }
    Ok(())
}

/// Deletes a session on Supabase (Admin only, enforced by RLS)
pub async fn delete_session_from_supabase(
    config: &SupabaseConfig,
    auth: &AuthSession,
    session_id: Uuid,
) -> Result<(), String> {
    if !config.is_configured() {
        return Ok(());
    }
    if !auth.can_manage_table() {
        return Err("Nur der Admin darf ganze Spielabende löschen.".to_string());
    }

    let base = config.project_url.trim_end_matches('/');
    let url = format!("{base}/rest/v1/sessions?id=eq.{session_id}");
    let resp = apply_supabase_headers(Request::delete(&url), config, Some(auth))
        .send()
        .await
        .map_err(|e| format!("Netzwerkfehler: {e}"))?;

    if !resp.ok() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("Löschen fehlgeschlagen: {txt}"));
    }
    Ok(())
}

/// Adds or updates a player in Supabase (Admin can add/edit anyone; player can update own color)
pub async fn upsert_player_to_supabase(
    config: &SupabaseConfig,
    auth: &AuthSession,
    player: &Player,
) -> Result<(), String> {
    if !config.is_configured() {
        return Ok(());
    }

    let base = config.project_url.trim_end_matches('/');
    let url = format!("{base}/rest/v1/players?on_conflict=id");
    let resp = apply_supabase_headers(Request::post(&url), config, Some(auth))
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates")
        .json(player)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| format!("Netzwerkfehler: {e}"))?;

    if !resp.ok() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("Spieler-Speicherfehler: {txt}"));
    }
    Ok(())
}
