/// 1970-01-01로부터의 일수. Howard Hinnant의 days_from_civil.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let mp = i64::from((month + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-10-15T05:28:29Z`·`…+09:00`·소수 초(`…29.123Z`)를 유닉스 초로 읽는다. 모양이 다르면 `None`.
pub fn parse_rfc3339(text: &str) -> Option<i64> {
    let (date, rest) = text.trim().split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let split_at = rest.find(['Z', '+', '-'])?;
    let (clock, zone) = rest.split_at(split_at);
    let mut clock_parts = clock.split(':');
    let hour: i64 = clock_parts.next()?.parse().ok()?;
    let minute: i64 = clock_parts.next()?.parse().ok()?;
    let second: i64 = clock_parts.next()?.split('.').next()?.parse().ok()?;

    let offset = match zone {
        "Z" => 0,
        zone => {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let (h, m) = zone[1..].split_once(':')?;
            sign * (h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60)
        }
    };

    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// 1970-01-01로부터의 일수 → (년, 월, 일). Howard Hinnant의 civil_from_days.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// 유닉스 초를 한국 시각 `10/15 14:03`으로.
pub fn seoul_short(secs: i64) -> String {
    let local = secs + 9 * 3600;
    let (_, month, day) = civil_from_days(local.div_euclid(86_400));
    let in_day = local.rem_euclid(86_400);
    format!("{month}/{day} {:02}:{:02}", in_day / 3600, (in_day % 3600) / 60)
}

/// SQLite `CURRENT_TIMESTAMP`(UTC, `2026-10-15 05:28:29`)를 한국 시각 `10/15 14:28`으로. 못 읽으면 원문 그대로.
pub fn sqlite_utc_to_seoul(text: &str) -> String {
    parse_rfc3339(&format!("{}Z", text.trim().replace(' ', "T")))
        .map(seoul_short)
        .unwrap_or_else(|| text.to_string())
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 걸린 시간을 사람이 읽게. `45초`, `12분`, `1시간 3분`, `2일`.
pub fn format_duration(secs: i64) -> String {
    let secs = secs.max(0);
    match secs {
        0..=59 => format!("{secs}초"),
        60..=3599 => format!("{}분", secs / 60),
        3600..=86_399 => match (secs % 3600) / 60 {
            0 => format!("{}시간", secs / 3600),
            minutes => format!("{}시간 {minutes}분", secs / 3600),
        },
        _ => format!("{}일", secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utc_offsets_and_fractions() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2026-10-15T05:28:29Z"), Some(1_792_042_109));
        assert_eq!(parse_rfc3339("2026-10-15T14:28:29+09:00"), Some(1_792_042_109));
        assert_eq!(parse_rfc3339("2026-10-14T22:58:29-06:30"), Some(1_792_042_109));
        assert_eq!(parse_rfc3339("2026-10-15T05:28:29.987Z"), Some(1_792_042_109));
    }

    #[test]
    fn rejects_malformed_timestamps() {
        for bad in ["", "yesterday", "2026-13-01T00:00:00Z", "2026-10-15", "2026-10-15T05:28Z", "2026-10-15T05:28:29"] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
    }

    #[test]
    fn utc_becomes_seoul_time() {
        assert_eq!(seoul_short(1_792_042_109), "10/15 14:28");
        assert_eq!(sqlite_utc_to_seoul("2026-10-15 05:28:29"), "10/15 14:28");
        assert_eq!(sqlite_utc_to_seoul("2026-10-15 15:00:00"), "10/16 00:00", "자정을 넘으면 날짜가 넘어간다");
        assert_eq!(sqlite_utc_to_seoul("???"), "???");
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(format_duration(-5), "0초");
        assert_eq!(format_duration(45), "45초");
        assert_eq!(format_duration(12 * 60 + 59), "12분");
        assert_eq!(format_duration(3600), "1시간");
        assert_eq!(format_duration(3600 + 3 * 60), "1시간 3분");
        assert_eq!(format_duration(2 * 86_400 + 5), "2일");
    }
}
