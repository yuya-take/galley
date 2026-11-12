//! 日時の表示（「今日 14:10」「昨日 16:20」「9月19日」）。
//!
//! タイムゾーンはサーバーのもの（環境変数 `TZ`、無ければ OS の設定）。

use jiff::{Timestamp, ToSpan, Zoned, tz::TimeZone};

/// いまの日時（サーバーのタイムゾーン）。
pub fn now() -> Zoned {
    Zoned::now()
}

/// 一覧などに出す短い日時。今日・昨日は時刻を、それより前は日付を出す（年が違えば年も）。
pub fn short(at: Timestamp, now: &Zoned) -> String {
    let at = at.to_zoned(now.time_zone().clone());
    let date = at.date();
    let today = now.date();
    if date == today {
        format!("今日 {:02}:{:02}", at.hour(), at.minute())
    } else if today.yesterday().is_ok_and(|yesterday| yesterday == date) {
        format!("昨日 {:02}:{:02}", at.hour(), at.minute())
    } else if date.year() == today.year() {
        format!("{}月{}日", date.month(), date.day())
    } else {
        format!("{}年{}月{}日", date.year(), date.month(), date.day())
    }
}

/// 日付と時刻（「9月20日 18:40」。年が違えば「2025年12月31日 12:00」）。過去の版の帯に使う。
pub fn medium(at: Timestamp, now: &Zoned) -> String {
    let zoned = at.to_zoned(now.time_zone().clone());
    if zoned.year() == now.year() {
        format!(
            "{}月{}日 {:02}:{:02}",
            zoned.month(),
            zoned.day(),
            zoned.hour(),
            zoned.minute()
        )
    } else {
        long(at, now.time_zone())
    }
}

/// 詳しい日時（「2026年9月19日 14:10」）。マウスを載せたときの表示に使う。
pub fn long(at: Timestamp, time_zone: &TimeZone) -> String {
    let at = at.to_zoned(time_zone.clone());
    format!(
        "{}年{}月{}日 {:02}:{:02}",
        at.year(),
        at.month(),
        at.day(),
        at.hour(),
        at.minute()
    )
}

/// `<time datetime>` に入れる形（RFC 3339、UTC、秒まで）。
pub fn machine(at: Timestamp) -> String {
    format!("{at:.0}")
}

/// 今週の始まり（月曜 0:00）。「今週更新」の絞り込みに使う。
pub fn start_of_week(now: &Zoned) -> Timestamp {
    let days_since_monday = i64::from(now.date().weekday().to_monday_zero_offset());
    now.date()
        .checked_sub(days_since_monday.days())
        .and_then(|monday| monday.to_zoned(now.time_zone().clone()))
        .map(|monday| monday.timestamp())
        // 暦の端など計算できないときは、7日前からにする
        .unwrap_or_else(|_| {
            now.timestamp()
                .checked_sub(168.hours())
                .unwrap_or(Timestamp::MIN)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokyo(text: &str) -> Zoned {
        format!("{text}[Asia/Tokyo]")
            .parse()
            .unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    fn at(text: &str) -> Timestamp {
        tokyo(text).timestamp()
    }

    #[test]
    fn short_uses_time_for_today_and_yesterday() {
        let now = tokyo("2026-09-21T10:00:00");
        assert_eq!(short(at("2026-09-21T09:05:00"), &now), "今日 09:05");
        assert_eq!(short(at("2026-09-20T16:20:00"), &now), "昨日 16:20");
        assert_eq!(short(at("2026-09-19T23:59:00"), &now), "9月19日");
        assert_eq!(short(at("2025-12-31T12:00:00"), &now), "2025年12月31日");
    }

    #[test]
    fn short_follows_the_time_zone_of_now() {
        // UTC では前日でも、東京では今日
        let now = tokyo("2026-09-21T08:00:00");
        let at: Timestamp = "2026-09-20T23:30:00Z"
            .parse()
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(short(at, &now), "今日 08:30");
    }

    #[test]
    fn medium_shows_date_and_time() {
        let now = tokyo("2026-09-21T10:00:00");
        assert_eq!(medium(at("2026-09-20T18:40:00"), &now), "9月20日 18:40");
        assert_eq!(medium(at("2026-09-21T09:05:00"), &now), "9月21日 09:05");
        assert_eq!(
            medium(at("2025-12-31T12:00:00"), &now),
            "2025年12月31日 12:00"
        );
    }

    #[test]
    fn long_and_machine_formats() {
        let tz = TimeZone::get("Asia/Tokyo").unwrap_or_else(|e| panic!("{e}"));
        let at = at("2026-09-19T14:10:30");
        assert_eq!(long(at, &tz), "2026年9月19日 14:10");
        assert_eq!(machine(at), "2026-09-19T05:10:30Z");
    }

    #[test]
    fn week_starts_on_monday() {
        // 2026-09-23 は水曜日
        assert_eq!(
            start_of_week(&tokyo("2026-09-23T15:00:00")),
            at("2026-09-21T00:00:00")
        );
        // 月曜日はその日の 0:00
        assert_eq!(
            start_of_week(&tokyo("2026-09-21T00:30:00")),
            at("2026-09-21T00:00:00")
        );
        // 日曜日は6日前の月曜日
        assert_eq!(
            start_of_week(&tokyo("2026-09-27T23:00:00")),
            at("2026-09-21T00:00:00")
        );
    }
}
