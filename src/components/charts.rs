use crate::models::{PlayerStats, format_date_de, format_euros};
use leptos::prelude::*;
use uuid::Uuid;

/// Interactive SVG Multi-Player Cumulative Bankroll Line Chart
#[component]
pub fn CumulativeBankrollChart(
    stats: Signal<Vec<PlayerStats>>,
    focused_player: ReadSignal<Option<Uuid>>,
) -> impl IntoView {
    let width = 760.0_f64;
    let height = 300.0_f64;
    let pad_left = 68.0_f64;
    let pad_right = 28.0_f64;
    let pad_top = 24.0_f64;
    let pad_bottom = 42.0_f64;

    let plot_w = width - pad_left - pad_right;
    let plot_h = height - pad_top - pad_bottom;

    view! {
        <div class="chart-card">
            <div class="chart-header">
                <h3>"📈 Kontostand über Zeit (Kumuliert)"</h3>
                <span class="chart-hint">"Start bei 0,00 € • Alle Spielabende chronologisch"</span>
            </div>

            {move || {
                let all_stats = stats.get();
                let num_sessions = all_stats
                    .first()
                    .map(|s| s.history_points.len())
                    .unwrap_or(0);

                if num_sessions == 0 {
                    return view! {
                        <div class="empty-chart">
                            "Noch keine Spielabende eingetragen. Trage deinen ersten Abend ein, um den Verlauf zu sehen!"
                        </div>
                    }.into_any();
                }

                // Find global min and max cents across all players (including 0)
                let mut min_c = 0_i64;
                let mut max_c = 0_i64;
                for st in &all_stats {
                    for pt in &st.history_points {
                        min_c = min_c.min(pt.cumulative_cents);
                        max_c = max_c.max(pt.cumulative_cents);
                    }
                }

                // Add 15% vertical breathing room
                let span = (max_c - min_c).max(500) as f64;
                let y_min = (min_c as f64) - span * 0.12;
                let y_max = (max_c as f64) + span * 0.12;
                let y_range = (y_max - y_min).max(100.0);

                let map_x = move |step_idx: usize| -> f64 {
                    // step_idx 0 is starting point (0.00 €), 1..=num_sessions are sessions
                    let total_steps = num_sessions.max(1) as f64;
                    pad_left + (step_idx as f64 / total_steps) * plot_w
                };

                let map_y = move |cents: f64| -> f64 {
                    pad_top + plot_h - ((cents - y_min) / y_range) * plot_h
                };

                let zero_y = map_y(0.0);

                // Y-axis grid ticks (5 ticks)
                let y_ticks: Vec<(f64, String)> = (0..=4)
                    .map(|i| {
                        let val = y_min + (i as f64 / 4.0) * y_range;
                        let rounded_cents = (val / 50.0).round() as i64 * 50;
                        (map_y(rounded_cents as f64), format_euros(rounded_cents, true))
                    })
                    .collect();

                // X-axis labels
                let x_labels: Vec<(f64, String)> = all_stats
                    .first()
                    .map(|s| {
                        let mut labels = vec![(map_x(0), "Start".to_string())];
                        let stride = (num_sessions / 6).max(1);
                        for (idx, pt) in s.history_points.iter().enumerate() {
                            let step = idx + 1;
                            if step == num_sessions || idx % stride == 0 {
                                labels.push((map_x(step), format_date_de(&pt.date)));
                            }
                        }
                        labels
                    })
                    .unwrap_or_default();

                let focus = focused_player.get();

                view! {
                    <div class="svg-wrapper">
                        <svg
                            viewBox=format!("0 0 {width} {height}")
                            class="line-chart-svg"
                            role="img"
                            aria-label="Kumulierter Gewinn- und Verlustverlauf über Zeit"
                        >
                            // Horizontal grid lines
                            {y_ticks.into_iter().map(|(y, label)| {
                                view! {
                                    <g>
                                        <line
                                            x1=pad_left
                                            y1=y
                                            x2=width - pad_right
                                            y2=y
                                            class="grid-line"
                                        />
                                        <text
                                            x=pad_left - 8.0
                                            y=y + 4.0
                                            text-anchor="end"
                                            class="axis-label"
                                        >
                                            {label}
                                        </text>
                                    </g>
                                }
                            }).collect_view()}

                            // Zero baseline
                            <line
                                x1=pad_left
                                y1=zero_y
                                x2=width - pad_right
                                y2=zero_y
                                class="zero-line"
                            />

                            // X-axis date labels
                            {x_labels.into_iter().map(|(x, label)| {
                                view! {
                                    <text
                                        x=x
                                        y=height - 10.0
                                        text-anchor="middle"
                                        class="axis-label"
                                    >
                                        {label}
                                    </text>
                                }
                            }).collect_view()}

                            // Player lines and data points
                            {all_stats.into_iter().map(|st| {
                                let is_dimmed = focus.is_some_and(|f_id| f_id != st.player_id);
                                let opacity = if is_dimmed { "0.22" } else { "1.0" };
                                let stroke_w = if focus == Some(st.player_id) { "3.5" } else { "2.5" };

                                let mut points_str = format!("{:.1},{:.1}", map_x(0), map_y(0.0));
                                let mut circles = vec![(
                                    map_x(0),
                                    map_y(0.0),
                                    format!("{}: Start (0,00 €)", st.player_name),
                                )];

                                for (idx, pt) in st.history_points.iter().enumerate() {
                                    let cx = map_x(idx + 1);
                                    let cy = map_y(pt.cumulative_cents as f64);
                                    points_str.push_str(&format!(" {cx:.1},{cy:.1}"));
                                    let delta_str = pt
                                        .delta_cents
                                        .map(|d| format!(" ({})", format_euros(d, true)))
                                        .unwrap_or_else(|| " (pausiert)".to_string());
                                    let tooltip = format!(
                                        "{} am {}: {}{}",
                                        st.player_name,
                                        format_date_de(&pt.date),
                                        format_euros(pt.cumulative_cents, true),
                                        delta_str
                                    );
                                    circles.push((cx, cy, tooltip));
                                }

                                let color = st.avatar_color.clone();

                                view! {
                                    <g style=format!("opacity: {opacity}; transition: opacity 0.2s;")>
                                        <polyline
                                            fill="none"
                                            stroke=color.clone()
                                            stroke-width=stroke_w
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            points=points_str
                                        />
                                        {circles.into_iter().map(|(cx, cy, tip)| {
                                            let c_fill = color.clone();
                                            view! {
                                                <circle
                                                    cx=cx
                                                    cy=cy
                                                    r="4.5"
                                                    fill=c_fill
                                                    stroke="#0f172a"
                                                    stroke-width="1.5"
                                                >
                                                    <title>{tip}</title>
                                                </circle>
                                            }
                                        }).collect_view()}
                                    </g>
                                }
                            }).collect_view()}
                        </svg>

                        // Legend
                        <div class="chart-legend">
                            {move || stats.get().into_iter().map(|st| {
                                let is_focused = focused_player.get() == Some(st.player_id);
                                view! {
                                    <span class={if is_focused { "legend-item active" } else { "legend-item" }}>
                                        <span
                                            class="legend-swatch"
                                            style=format!("background-color: {};", st.avatar_color)
                                        ></span>
                                        <span class="legend-name">{st.player_name}</span>
                                        <strong class={if st.total_cents >= 0 { "pos-text" } else { "neg-text" }}>
                                            {format_euros(st.total_cents, true)}
                                        </strong>
                                    </span>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }.into_any()
            }}
        </div>
    }
}

/// Session-by-Session Win/Loss Bar Chart for the selected player
#[component]
pub fn PlayerSessionBarsChart(
    stats: Signal<Vec<PlayerStats>>,
    focused_player: ReadSignal<Option<Uuid>>,
) -> impl IntoView {
    let width = 760.0_f64;
    let height = 240.0_f64;
    let pad_left = 68.0_f64;
    let pad_right = 24.0_f64;
    let pad_top = 20.0_f64;
    let pad_bottom = 38.0_f64;

    let plot_w = width - pad_left - pad_right;
    let plot_h = height - pad_top - pad_bottom;

    view! {
        <div class="chart-card">
            {move || {
                let all_stats = stats.get();
                let target = focused_player
                    .get()
                    .and_then(|id| all_stats.iter().find(|s| s.player_id == id).cloned())
                    .or_else(|| all_stats.first().cloned());

                let Some(player) = target else {
                    return view! { <div class="empty-chart">"Kein Spieler ausgewählt."</div> }.into_any();
                };

                let played_points: Vec<_> = player
                    .history_points
                    .iter()
                    .filter_map(|pt| pt.delta_cents.map(|d| (pt.date.clone(), d)))
                    .collect();

                if played_points.is_empty() {
                    return view! {
                        <div class="empty-chart">
                            {format!("Noch keine gespielten Abende für {}.", player.player_name)}
                        </div>
                    }.into_any();
                }

                let max_abs = played_points
                    .iter()
                    .map(|(_, d)| d.abs())
                    .max()
                    .unwrap_or(500)
                    .max(200) as f64
                    * 1.15;

                let zero_y = pad_top + plot_h / 2.0;
                let n = played_points.len() as f64;
                let slot_w = plot_w / n;
                let bar_w = (slot_w * 0.62).clamp(8.0, 48.0);

                view! {
                    <div>
                        <div class="chart-header">
                            <h3>{format!("📊 Einzelabende: {}", player.player_name)}</h3>
                            <span class="chart-hint">"Gewinn / Verlust pro Spielabend"</span>
                        </div>
                        <div class="svg-wrapper">
                            <svg
                                viewBox=format!("0 0 {width} {height}")
                                class="line-chart-svg"
                                role="img"
                                aria-label="Gewinn und Verlust pro Abend"
                            >
                                // Top, Zero, Bottom reference lines
                                <line x1=pad_left y1=pad_top x2=width - pad_right y2=pad_top class="grid-line" />
                                <text x=pad_left - 8.0 y=pad_top + 4.0 text-anchor="end" class="axis-label">
                                    {format_euros(max_abs.round() as i64, true)}
                                </text>

                                <line x1=pad_left y1=zero_y x2=width - pad_right y2=zero_y class="zero-line" />
                                <text x=pad_left - 8.0 y=zero_y + 4.0 text-anchor="end" class="axis-label">
                                    "0,00 €"
                                </text>

                                <line x1=pad_left y1=pad_top + plot_h x2=width - pad_right y2=pad_top + plot_h class="grid-line" />
                                <text x=pad_left - 8.0 y=pad_top + plot_h + 4.0 text-anchor="end" class="axis-label">
                                    {format_euros(-(max_abs.round() as i64), true)}
                                </text>

                                {played_points.into_iter().enumerate().map(|(idx, (date, delta))| {
                                    let cx = pad_left + (idx as f64 + 0.5) * slot_w;
                                    let x = cx - bar_w / 2.0;
                                    let h = ((delta.abs() as f64) / max_abs) * (plot_h / 2.0);
                                    let h_clamped = h.max(2.0);
                                    let y = if delta >= 0 { zero_y - h_clamped } else { zero_y };
                                    let fill = if delta >= 0 { "#10b981" } else { "#ef4444" };
                                    let tip = format!("{}: {}", format_date_de(&date), format_euros(delta, true));

                                    view! {
                                        <g>
                                            <rect
                                                x=x
                                                y=y
                                                width=bar_w
                                                height=h_clamped
                                                rx="4"
                                                fill=fill
                                            >
                                                <title>{tip}</title>
                                            </rect>
                                            <text
                                                x=cx
                                                y=height - 10.0
                                                text-anchor="middle"
                                                class="axis-label"
                                            >
                                                {format_date_de(&date)}
                                            </text>
                                        </g>
                                    }
                                }).collect_view()}
                            </svg>
                        </div>
                    </div>
                }.into_any()
            }}
        </div>
    }
}
