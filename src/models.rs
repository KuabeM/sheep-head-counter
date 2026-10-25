use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const DEFAULT_COLORS: &[&str] = &[
    "#10b981", // Emerald
    "#3b82f6", // Blue
    "#f59e0b", // Amber
    "#ec4899", // Pink
    "#8b5cf6", // Violet
    "#06b6d4", // Cyan
    "#ef4444", // Red
    "#84cc16", // Lime
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub id: Uuid,
    pub name: String,
    pub avatar_color: String,
    #[serde(default = "default_true")]
    pub active: bool,
    /// Supabase Auth user UUID (`auth.users.id`) who claimed this player profile
    #[serde(default)]
    pub claimed_by: Option<Uuid>,
    /// True if this player has Admin privileges (can edit all players' results, delete sessions, manage roster)
    #[serde(default)]
    pub is_admin: bool,
}

fn default_true() -> bool {
    true
}

impl Player {
    pub fn new(name: impl Into<String>, color_idx: usize, is_admin: bool) -> Self {
        let color = DEFAULT_COLORS[color_idx % DEFAULT_COLORS.len()].to_string();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            avatar_color: color,
            active: true,
            claimed_by: None,
            is_admin,
        }
    }
}

/// Represents the currently logged-in user and their claimed player identity
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthSession {
    /// Supabase `auth.users.id` (or local mock UUID in offline mode)
    pub user_id: Option<Uuid>,
    pub email: Option<String>,
    pub access_token: Option<String>,
    /// The `Player.id` this user has chosen/claimed
    pub claimed_player_id: Option<Uuid>,
    /// True if the claimed player has admin rights
    pub is_admin: bool,
}

impl AuthSession {
    pub fn is_logged_in(&self) -> bool {
        self.user_id.is_some()
    }

    /// Returns true if the user is authenticated (has a valid Supabase token when Supabase is configured, or local login when offline)
    pub fn is_authenticated(&self, supabase_configured: bool) -> bool {
        if supabase_configured {
            self.user_id.is_some()
                && self
                    .access_token
                    .as_deref()
                    .is_some_and(|t| !t.trim().is_empty())
        } else {
            self.user_id.is_some()
        }
    }

    /// Returns true if the current user is allowed to edit a specific player's score
    pub fn can_edit_player(&self, target_player_id: &Uuid) -> bool {
        if self.is_admin {
            return true;
        }
        self.claimed_player_id.as_ref() == Some(target_player_id)
    }

    /// Returns true if the current user can manage the table (add/remove players, delete full sessions, edit others)
    pub fn can_manage_table(&self) -> bool {
        self.is_admin
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionResult {
    pub player_id: Uuid,
    /// Net amount won or lost in cents (e.g. +1450 = +14.50 €, -620 = -6.20 €)
    pub amount_cents: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSession {
    pub id: Uuid,
    /// ISO date string YYYY-MM-DD
    pub played_at: String,
    /// Duration played in minutes (optional, e.g. 180 for 3 hours)
    pub duration_minutes: Option<u32>,
    /// Optional note or location (e.g. "Stammtisch beim Wirt, 10/20 Cent")
    #[serde(default)]
    pub note: String,
    /// End-of-evening results for each participating player
    #[serde(default)]
    pub results: Vec<SessionResult>,
}

impl GameSession {
    pub fn total_balance_cents(&self) -> i64 {
        self.results.iter().map(|r| r.amount_cents).sum()
    }

    pub fn is_zero_sum(&self) -> bool {
        self.total_balance_cents() == 0
    }

    pub fn result_for_player(&self, player_id: &Uuid) -> Option<i64> {
        self.results
            .iter()
            .find(|r| &r.player_id == player_id)
            .map(|r| r.amount_cents)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableData {
    pub table_slug: String,
    pub players: Vec<Player>,
    pub sessions: Vec<GameSession>,
}

impl Default for TableData {
    fn default() -> Self {
        let p1 = Player::new("Korbinian", 0, true);
        let p2 = Player::new("Max", 1, false);
        let p3 = Player::new("Lukas", 2, false);
        let p4 = Player::new("Sepp", 3, false);

        Self {
            table_slug: "stammtisch".to_string(),
            players: vec![p1, p2, p3, p4],
            sessions: Vec::new(),
        }
    }
}

impl TableData {
    /// Returns sessions sorted chronologically (oldest first) for cumulative series
    pub fn sessions_chronological(&self) -> Vec<GameSession> {
        let mut sorted = self.sessions.clone();
        sorted.sort_by(|a, b| a.played_at.cmp(&b.played_at).then(a.id.cmp(&b.id)));
        sorted
    }

    /// Returns sessions sorted reverse-chronologically (newest first) for history view
    pub fn sessions_newest_first(&self) -> Vec<GameSession> {
        let mut sorted = self.sessions_chronological();
        sorted.reverse();
        sorted
    }

    pub fn player_by_id(&self, id: &Uuid) -> Option<&Player> {
        self.players.iter().find(|p| &p.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerStats {
    pub player_id: Uuid,
    pub player_name: String,
    pub avatar_color: String,
    pub is_admin: bool,
    pub sessions_played: usize,
    pub wins: usize,
    pub losses: usize,
    pub draws: usize,
    pub total_cents: i64,
    pub total_minutes: u32,
    pub best_session_cents: i64,
    pub worst_session_cents: i64,
    /// Positive for winning streak, negative for losing streak
    pub current_streak: i32,
    /// Cumulative bankroll after each session the table played
    pub history_points: Vec<HistoryPoint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub session_id: Uuid,
    pub date: String,
    pub cumulative_cents: i64,
    pub delta_cents: Option<i64>,
}

impl PlayerStats {
    pub fn win_ratio_percent(&self) -> f64 {
        if self.sessions_played == 0 {
            0.0
        } else {
            (self.wins as f64 / self.sessions_played as f64) * 100.0
        }
    }

    pub fn avg_cents_per_session(&self) -> f64 {
        if self.sessions_played == 0 {
            0.0
        } else {
            self.total_cents as f64 / self.sessions_played as f64
        }
    }

    pub fn cents_per_hour(&self) -> Option<f64> {
        if self.total_minutes == 0 {
            None
        } else {
            let hours = self.total_minutes as f64 / 60.0;
            Some(self.total_cents as f64 / hours)
        }
    }
}

/// Computes comprehensive statistics for all players in the table
pub fn compute_all_stats(data: &TableData) -> Vec<PlayerStats> {
    let chronological = data.sessions_chronological();
    let mut stats_map: HashMap<Uuid, PlayerStats> = data
        .players
        .iter()
        .map(|p| {
            (
                p.id,
                PlayerStats {
                    player_id: p.id,
                    player_name: p.name.clone(),
                    avatar_color: p.avatar_color.clone(),
                    is_admin: p.is_admin,
                    sessions_played: 0,
                    wins: 0,
                    losses: 0,
                    draws: 0,
                    total_cents: 0,
                    total_minutes: 0,
                    best_session_cents: 0,
                    worst_session_cents: 0,
                    current_streak: 0,
                    history_points: Vec::new(),
                },
            )
        })
        .collect();

    for session in &chronological {
        for player in &data.players {
            if let Some(st) = stats_map.get_mut(&player.id) {
                let delta = session.result_for_player(&player.id);
                if let Some(amount) = delta {
                    if st.sessions_played == 0 {
                        st.best_session_cents = amount;
                        st.worst_session_cents = amount;
                    } else {
                        st.best_session_cents = st.best_session_cents.max(amount);
                        st.worst_session_cents = st.worst_session_cents.min(amount);
                    }

                    st.sessions_played += 1;
                    st.total_cents += amount;

                    if let Some(mins) = session.duration_minutes {
                        st.total_minutes += mins;
                    }

                    if amount > 0 {
                        st.wins += 1;
                        st.current_streak = if st.current_streak >= 0 {
                            st.current_streak + 1
                        } else {
                            1
                        };
                    } else if amount < 0 {
                        st.losses += 1;
                        st.current_streak = if st.current_streak <= 0 {
                            st.current_streak - 1
                        } else {
                            -1
                        };
                    } else {
                        st.draws += 1;
                        st.current_streak = 0;
                    }
                }

                st.history_points.push(HistoryPoint {
                    session_id: session.id,
                    date: session.played_at.clone(),
                    cumulative_cents: st.total_cents,
                    delta_cents: delta,
                });
            }
        }
    }

    let mut list: Vec<PlayerStats> = data
        .players
        .iter()
        .filter_map(|p| stats_map.remove(&p.id))
        .collect();

    list.sort_by(|a, b| {
        b.total_cents
            .cmp(&a.total_cents)
            .then_with(|| b.wins.cmp(&a.wins))
            .then_with(|| a.player_name.cmp(&b.player_name))
    });

    list
}

/// Formats integer cents into a German/European currency string like "+14,50 €" or "-6,20 €"
pub fn format_euros(cents: i64, show_plus: bool) -> String {
    let sign = if cents > 0 && show_plus {
        "+"
    } else if cents < 0 {
        "-"
    } else {
        ""
    };
    let abs = cents.unsigned_abs();
    let euros = abs / 100;
    let rem_cents = abs % 100;
    format!("{sign}{euros},{rem_cents:02} €")
}

/// Parses a user input string (accepting both "14,50" and "14.50" or "-6.2" or "15") into cents
pub fn parse_euros_to_cents(input: &str) -> Option<i64> {
    let trimmed = input.trim().replace('€', "").replace(' ', "");
    if trimmed.is_empty() {
        return Some(0);
    }
    let normalized = trimmed.replace(',', ".");
    let val: f64 = normalized.parse().ok()?;
    if !val.is_finite() {
        return None;
    }
    Some((val * 100.0).round() as i64)
}

/// Formats an ISO date YYYY-MM-DD into DD.MM.YYYY if valid
pub fn format_date_de(iso_date: &str) -> String {
    if let Ok(d) = NaiveDate::parse_from_str(iso_date, "%Y-%m-%d") {
        d.format("%d.%m.%Y").to_string()
    } else {
        iso_date.to_string()
    }
}

/// Creates realistic sample sessions for quick demo/exploration
pub fn create_demo_data() -> TableData {
    let p1 = Player::new("Korbinian", 0, true);
    let p2 = Player::new("Max", 1, false);
    let p3 = Player::new("Lukas", 2, false);
    let p4 = Player::new("Sepp", 3, false);

    let sessions = vec![
        GameSession {
            id: Uuid::new_v4(),
            played_at: "2026-09-18".to_string(),
            duration_minutes: Some(180),
            note: "Beim Wirt – 10/20/50 Cent".to_string(),
            results: vec![
                SessionResult { player_id: p1.id, amount_cents: 840 },
                SessionResult { player_id: p2.id, amount_cents: -320 },
                SessionResult { player_id: p3.id, amount_cents: 190 },
                SessionResult { player_id: p4.id, amount_cents: -710 },
            ],
        },
        GameSession {
            id: Uuid::new_v4(),
            played_at: "2026-09-25".to_string(),
            duration_minutes: Some(210),
            note: "Bei Max daheim".to_string(),
            results: vec![
                SessionResult { player_id: p1.id, amount_cents: -450 },
                SessionResult { player_id: p2.id, amount_cents: 1120 },
                SessionResult { player_id: p3.id, amount_cents: -280 },
                SessionResult { player_id: p4.id, amount_cents: -390 },
            ],
        },
        GameSession {
            id: Uuid::new_v4(),
            played_at: "2026-10-02".to_string(),
            duration_minutes: Some(240),
            note: "Langer Abend mit Bockrunden".to_string(),
            results: vec![
                SessionResult { player_id: p1.id, amount_cents: 1650 },
                SessionResult { player_id: p2.id, amount_cents: -540 },
                SessionResult { player_id: p3.id, amount_cents: -890 },
                SessionResult { player_id: p4.id, amount_cents: -220 },
            ],
        },
        GameSession {
            id: Uuid::new_v4(),
            played_at: "2026-10-16".to_string(),
            duration_minutes: Some(195),
            note: "Stammtisch".to_string(),
            results: vec![
                SessionResult { player_id: p1.id, amount_cents: 620 },
                SessionResult { player_id: p2.id, amount_cents: -410 },
                SessionResult { player_id: p3.id, amount_cents: 330 },
                SessionResult { player_id: p4.id, amount_cents: -540 },
            ],
        },
    ];

    TableData {
        table_slug: "stammtisch".to_string(),
        players: vec![p1, p2, p3, p4],
        sessions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_format_euros() {
        assert_eq!(parse_euros_to_cents("14,50"), Some(1450));
        assert_eq!(parse_euros_to_cents("-6.20"), Some(-620));
        assert_eq!(parse_euros_to_cents("+8"), Some(800));
        assert_eq!(parse_euros_to_cents(""), Some(0));

        assert_eq!(format_euros(1450, true), "+14,50 €");
        assert_eq!(format_euros(-620, true), "-6,20 €");
        assert_eq!(format_euros(0, true), "0,00 €");
    }

    #[test]
    fn test_permissions_model() {
        let p_korb = Uuid::new_v4();
        let p_max = Uuid::new_v4();

        // Regular player (Max) can only edit Max's own score
        let max_auth = AuthSession {
            user_id: Some(Uuid::new_v4()),
            email: Some("max@example.com".to_string()),
            access_token: None,
            claimed_player_id: Some(p_max),
            is_admin: false,
        };
        assert!(max_auth.can_edit_player(&p_max));
        assert!(!max_auth.can_edit_player(&p_korb));
        assert!(!max_auth.can_manage_table());

        // Admin (Korbinian) can edit everyone and manage table
        let admin_auth = AuthSession {
            user_id: Some(Uuid::new_v4()),
            email: Some("korbinian@example.com".to_string()),
            access_token: None,
            claimed_player_id: Some(p_korb),
            is_admin: true,
        };
        assert!(admin_auth.can_edit_player(&p_max));
        assert!(admin_auth.can_edit_player(&p_korb));
        assert!(admin_auth.can_manage_table());
    }

    #[test]
    fn test_demo_data_zero_sum_and_stats() {
        let data = create_demo_data();
        for s in &data.sessions {
            assert!(s.is_zero_sum(), "Session {} should be zero-sum", s.played_at);
        }

        let stats = compute_all_stats(&data);
        assert_eq!(stats.len(), 4);

        let korb = stats.iter().find(|s| s.player_name == "Korbinian").unwrap();
        assert_eq!(korb.total_cents, 2660);
        assert!(korb.is_admin);
        assert_eq!(korb.sessions_played, 4);
        assert_eq!(korb.wins, 3);
        assert_eq!(korb.losses, 1);
        assert_eq!(korb.current_streak, 2);
        assert!((korb.win_ratio_percent() - 75.0).abs() < 1e-6);
    }
}
