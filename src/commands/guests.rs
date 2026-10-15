use crate::cms::{Cms, Guest, GuestList};
use crate::error::AppError;
use crate::timeutil::sqlite_utc_to_seoul;

/// 전시가 12시에 열고 5시에 닫는다. lovelog의 `VISIT_HOURS`, cms-api의 `VISIT_HOUR_MIN/MAX`, tyange-cms
/// `wedding-guests/index.vue`와 같은 범위다. **전시 시간이 바뀌면 네 곳을 함께 고친다.**
pub const VISIT_HOURS: [i64; 5] = [12, 13, 14, 15, 16];

fn clock(hour24: i64) -> i64 {
    if hour24 > 12 {
        hour24 - 12
    } else {
        hour24
    }
}

pub fn hour_label(hour: i64) -> String {
    format!("{}-{}시", clock(hour), clock(hour + 1))
}

fn head_count(guest: &Guest) -> i64 {
    guest.companions + 1
}

#[derive(Debug, PartialEq, Eq)]
pub struct HourRow {
    pub label: String,
    pub groups: usize,
    pub attendees: i64,
}

/// 시간대별 인원. 데이터에 없는 칸도 0으로 보여 주고(비어 있는 시간대가 보여야 쓸모가 있다), 예상 밖의 시각이
/// 저장돼 있어도 조용히 사라지지 않게 함께 모은다. 시간대를 받기 전의 하객은 맨 아래 "시간 미정".
pub fn breakdown(list: &GuestList) -> Vec<HourRow> {
    let mut hours: Vec<i64> = VISIT_HOURS.to_vec();
    for guest in &list.guests {
        if let Some(hour) = guest.visit_hour {
            if !hours.contains(&hour) {
                hours.push(hour);
            }
        }
    }
    hours.sort_unstable();

    let mut rows: Vec<HourRow> = hours
        .into_iter()
        .map(|hour| {
            let matched: Vec<&Guest> = list.guests.iter().filter(|g| g.visit_hour == Some(hour)).collect();
            HourRow {
                label: hour_label(hour),
                groups: matched.len(),
                attendees: matched.iter().map(|g| head_count(g)).sum(),
            }
        })
        .collect();

    let undecided: Vec<&Guest> = list.guests.iter().filter(|g| g.visit_hour.is_none()).collect();
    if !undecided.is_empty() {
        rows.push(HourRow {
            label: "시간 미정".into(),
            groups: undecided.len(),
            attendees: undecided.iter().map(|g| head_count(g)).sum(),
        });
    }
    rows
}

pub fn bar(attendees: i64, max: i64, width: usize) -> String {
    if max <= 0 || attendees <= 0 {
        return String::new();
    }
    let filled = ((attendees as f64 / max as f64) * width as f64).round().max(1.0) as usize;
    "█".repeat(filled.min(width))
}

pub fn run(cms: &Cms, json: bool) -> Result<(), AppError> {
    let list = cms.guests()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    if list.guests.is_empty() {
        println!("아직 접수된 하객이 없습니다.");
        return Ok(());
    }

    println!("신청 {}건 · 총 {}명 (동반 포함)\n", list.total_guests, list.total_attendees);
    let rows = breakdown(&list);
    let max = rows.iter().map(|r| r.attendees).max().unwrap_or(0);
    println!("시간대별 인원");
    for row in &rows {
        println!(
            "  {:<8} {:<20} {}명 · {}건",
            row.label,
            bar(row.attendees, max, 20),
            row.attendees,
            row.groups
        );
    }

    println!("\n명단 (최신순)");
    for guest in &list.guests {
        let companions = if guest.companions > 0 {
            format!("외 {}명", guest.companions)
        } else {
            String::new()
        };
        let hour = guest.visit_hour.map(hour_label).unwrap_or_else(|| "시간 미정".into());
        println!(
            "  {:<11} {:<12} {:<7} {}",
            sqlite_utc_to_seoul(&guest.created_at),
            guest.name,
            companions,
            hour
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guest(id: i64, companions: i64, hour: Option<i64>) -> Guest {
        Guest {
            guest_id: id,
            name: format!("손님{id}"),
            companions,
            visit_hour: hour,
            created_at: "2026-10-01 03:00:00".into(),
        }
    }

    fn list(guests: Vec<Guest>) -> GuestList {
        GuestList { total_guests: guests.len() as i64, total_attendees: guests.iter().map(head_count).sum(), guests }
    }

    #[test]
    fn hours_read_like_the_cms_page() {
        assert_eq!(hour_label(12), "12-1시");
        assert_eq!(hour_label(16), "4-5시");
        assert_eq!(hour_label(11), "11-12시");
    }

    #[test]
    fn empty_hours_still_show_as_zero() {
        let rows = breakdown(&list(vec![guest(1, 0, Some(13))]));
        assert_eq!(rows.len(), VISIT_HOURS.len());
        assert_eq!(rows[0], HourRow { label: "12-1시".into(), groups: 0, attendees: 0 });
        assert_eq!(rows[1], HourRow { label: "1-2시".into(), groups: 1, attendees: 1 });
    }

    #[test]
    fn companions_count_toward_attendees() {
        let rows = breakdown(&list(vec![guest(1, 2, Some(12)), guest(2, 0, Some(12))]));
        assert_eq!(rows[0], HourRow { label: "12-1시".into(), groups: 2, attendees: 4 });
    }

    #[test]
    fn unexpected_hours_and_undecided_guests_are_not_dropped() {
        let rows = breakdown(&list(vec![guest(1, 0, Some(10)), guest(2, 1, None)]));
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels[0], "10-11시", "예상 밖 시각도 정렬돼 앞에 나온다");
        assert_eq!(*labels.last().unwrap(), "시간 미정");
        assert_eq!(rows.last().unwrap().attendees, 2);
    }

    #[test]
    fn bars_scale_to_the_busiest_hour() {
        assert_eq!(bar(10, 10, 20).chars().count(), 20);
        assert_eq!(bar(5, 10, 20).chars().count(), 10);
        assert_eq!(bar(1, 100, 20).chars().count(), 1, "0이 아니면 최소 한 칸");
        assert_eq!(bar(0, 10, 20), "");
        assert_eq!(bar(3, 0, 20), "");
    }
}
