use chrono::{DateTime, SecondsFormat, Utc};
use chrono_tz::Tz;
use std::sync::OnceLock;

static SYSTEM_TIMEZONE: OnceLock<Tz> = OnceLock::new();

fn parse_timezone_name(name: &str) -> Option<Tz> {
    let name = name.strip_prefix(':').unwrap_or(name);
    let name = name.strip_prefix("/usr/share/zoneinfo/").unwrap_or(name);
    name.parse().ok()
}

/// Resolve the process timezone once. Home Assistant supplies `TZ`; standalone
/// installations fall back to the operating system's IANA timezone and finally UTC.
pub fn system_timezone() -> Tz {
    *SYSTEM_TIMEZONE.get_or_init(|| {
        std::env::var("TZ")
            .ok()
            .as_deref()
            .and_then(parse_timezone_name)
            .or_else(|| {
                iana_time_zone::get_timezone()
                    .ok()
                    .as_deref()
                    .and_then(parse_timezone_name)
            })
            .unwrap_or(chrono_tz::UTC)
    })
}

pub fn now() -> DateTime<Tz> {
    Utc::now().with_timezone(&system_timezone())
}

pub fn now_rfc3339() -> String {
    now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::parse_timezone_name;

    #[test]
    fn accepts_iana_and_zoneinfo_timezone_names() {
        assert_eq!(
            parse_timezone_name("Europe/Berlin").map(|tz| tz.name()),
            Some("Europe/Berlin")
        );
        assert_eq!(
            parse_timezone_name(":/usr/share/zoneinfo/America/New_York").map(|tz| tz.name()),
            Some("America/New_York")
        );
    }

    #[test]
    fn rejects_invalid_timezone_names() {
        assert!(parse_timezone_name("not/a/timezone").is_none());
    }
}
