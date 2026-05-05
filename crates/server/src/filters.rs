use chrono::{DateTime, FixedOffset, NaiveDateTime};

pub fn format_date(date: &str) -> String {
    if let Ok(dt) = DateTime::<FixedOffset>::parse_from_rfc3339(date) {
        return dt.format("%B %-d, %Y").to_string();
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S") {
        return dt.format("%B %-d, %Y").to_string();
    }
    if let Ok(dt) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        return dt.format("%B %-d, %Y").to_string();
    }
    date.to_string()
}
