use ratatui::layout::{Constraint, Layout, Rect};

/// Builds an ASCII battery gauge.
#[must_use]
pub fn bar(pct: u8, width: usize) -> String {
    let filled = (usize::from(pct) * width + 50) / 100;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// Centers a rectangle by percentage of the parent area.
#[must_use]
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let pad_y = (100 - percent_y) / 2;
    let pad_x = (100 - percent_x) / 2;
    let rows = Layout::vertical([
        Constraint::Percentage(pad_y),
        Constraint::Percentage(percent_y),
        Constraint::Percentage(pad_y),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage(pad_x),
        Constraint::Percentage(percent_x),
        Constraint::Percentage(pad_x),
    ])
    .split(rows[1])[1]
}

/// A horizontally proportional, fixed-height modal box.
#[must_use]
pub fn centered_box(width_pct: u16, height: u16, area: Rect) -> Rect {
    let width = area.width.saturating_mul(width_pct) / 100;
    let height = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

/// Truncates to `max` display cells, appending an ellipsis when needed.
#[must_use]
pub fn truncate(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut chars = text.chars();
    let mut out: String = chars.by_ref().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Keeps the trailing `max` characters, used for live text input.
#[must_use]
pub fn tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    text.chars().skip(count - max).collect()
}

/// Steps a bounded value while staying inside `min..=max`.
#[must_use]
pub fn step(value: u8, delta: i32, stride: u8, min: u8, max: u8) -> u8 {
    let direction = if delta < 0 { -1 } else { 1 };
    let next = i32::from(value) + direction * i32::from(stride);
    let clamped = next.clamp(i32::from(min), i32::from(max));
    u8::try_from(clamped).unwrap_or(value)
}
