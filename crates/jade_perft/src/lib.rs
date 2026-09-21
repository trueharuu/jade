pub mod model;
pub mod perft;

pub fn human(value: f64) -> String {
    if value < 1_000.0 {
        format!("{value:.2}")
    } else if value < 1_000_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else if value < 1_000_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value < 1_000_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else {
        format!("{:.2}T", value / 1_000_000_000_000.0)
    }
}
