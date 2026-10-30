//! Listening statistics from per-day totals.
//!
//! Days are local calendar days, already worked out by the caller (the
//! database knows the time zone). This module only does calendar arithmetic
//! and sums, so it is fully deterministic.

/// A calendar date (proleptic Gregorian).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// Parses "YYYY-MM-DD"; `None` for anything else or impossible dates.
    pub fn parse(s: &str) -> Option<Date> {
        let mut it = s.trim().splitn(3, '-');
        let year = it.next()?.parse().ok()?;
        let month = it.next()?.parse().ok()?;
        let day = it.next()?.parse().ok()?;
        let d = Date { year, month, day };
        (Date::from_days(d.days()) == d).then_some(d)
    }

    /// Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    pub fn days(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_days(days: i64) -> Date {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Date { year, month, day }
    }

    pub fn add_days(self, n: i64) -> Date {
        Date::from_days(self.days() + n)
    }

    /// 0 = Monday ... 6 = Sunday.
    pub fn weekday(self) -> u32 {
        // 1970-01-01 was a Thursday (3).
        (self.days() + 3).rem_euclid(7) as u32
    }

    /// The Monday starting this date's week.
    pub fn week_start(self) -> Date {
        self.add_days(-i64::from(self.weekday()))
    }

    pub fn month_start(self) -> Date {
        Date { day: 1, ..self }
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub today_ms: i64,
    /// Since Monday.
    pub week_ms: i64,
    /// Since the 1st of the month.
    pub month_ms: i64,
    pub total_ms: i64,
    /// Consecutive days with listening, ending today, or yesterday when
    /// nothing was played yet today (the streak is not broken until the day ends).
    pub streak_days: u32,
    pub best_streak_days: u32,
    /// The last `days` days, oldest first, including days without listening.
    pub daily: Vec<(Date, i64)>,
}

/// Summarizes per-day listening (`(day, ms)`, any order, days may repeat).
pub fn summarize(per_day: &[(Date, i64)], today: Date, days: usize) -> Summary {
    use std::collections::BTreeMap;
    let mut by_day: BTreeMap<Date, i64> = BTreeMap::new();
    for &(d, ms) in per_day {
        if ms > 0 {
            *by_day.entry(d).or_default() += ms;
        }
    }
    let sum_since = |from: Date| by_day.range(from..=today).map(|(_, ms)| ms).sum::<i64>();

    let mut streak = 0u32;
    let mut day = if by_day.contains_key(&today) {
        today
    } else {
        today.add_days(-1)
    };
    while by_day.contains_key(&day) {
        streak += 1;
        day = day.add_days(-1);
    }
    let mut best = 0u32;
    let mut run = 0u32;
    let mut prev: Option<Date> = None;
    for &d in by_day.keys() {
        run = if prev.is_some_and(|p| p.add_days(1) == d) {
            run + 1
        } else {
            1
        };
        best = best.max(run);
        prev = Some(d);
    }

    let daily = (0..days as i64)
        .rev()
        .map(|back| {
            let d = today.add_days(-back);
            (d, by_day.get(&d).copied().unwrap_or(0))
        })
        .collect();

    Summary {
        today_ms: by_day.get(&today).copied().unwrap_or(0),
        week_ms: sum_since(today.week_start()),
        month_ms: sum_since(today.month_start()),
        total_ms: by_day.values().sum(),
        streak_days: streak,
        best_streak_days: best,
        daily,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        Date::parse(s).expect("valid date")
    }

    #[test]
    fn calendar_arithmetic() {
        assert_eq!(d("1970-01-01").days(), 0);
        assert_eq!(d("2000-03-01").days(), 11_017);
        assert_eq!(Date::from_days(11_017), d("2000-03-01"));
        assert_eq!(d("2024-02-28").add_days(1), d("2024-02-29"), "leap year");
        assert_eq!(d("2023-02-28").add_days(1), d("2023-03-01"));
        assert_eq!(d("2026-12-31").add_days(1), d("2027-01-01"));
        assert_eq!(d("2026-10-30").weekday(), 4, "a Friday");
        assert_eq!(d("2026-10-30").week_start(), d("2026-10-26"));
        assert_eq!(
            d("2026-10-26").week_start(),
            d("2026-10-26"),
            "Monday is its own week start"
        );
        assert_eq!(
            d("2026-11-01").week_start(),
            d("2026-10-26"),
            "Sunday ends the week"
        );
        assert_eq!(d("2026-10-30").month_start(), d("2026-10-01"));
        assert_eq!(d("2026-10-30").to_string(), "2026-10-30");
        for bad in [
            "2026-02-30",
            "2026-13-01",
            "26-1",
            "",
            "x-y-z",
            "2026-00-10",
        ] {
            assert_eq!(Date::parse(bad), None, "{bad}");
        }
        // Round trip over a long span.
        for n in (-800_000..800_000).step_by(997) {
            assert_eq!(Date::from_days(n).days(), n);
        }
    }

    #[test]
    fn totals_for_periods() {
        let today = d("2026-10-30"); // Friday
        let s = summarize(
            &[
                (d("2026-10-30"), 1_000),
                (d("2026-10-30"), 500),    // same day twice: summed
                (d("2026-10-26"), 2_000),  // Monday this week
                (d("2026-10-25"), 4_000),  // Sunday last week, still this month
                (d("2026-09-30"), 8_000),  // last month
                (d("2026-11-02"), 16_000), // future (clock change): ignored for periods
                (d("2026-10-29"), 0),      // zero: not a listening day
            ],
            today,
            7,
        );
        assert_eq!(s.today_ms, 1_500);
        assert_eq!(s.week_ms, 3_500);
        assert_eq!(s.month_ms, 7_500);
        assert_eq!(s.total_ms, 31_500);
        assert_eq!(s.daily.len(), 7);
        assert_eq!(s.daily[0], (d("2026-10-24"), 0));
        assert_eq!(s.daily[6], (d("2026-10-30"), 1_500));
    }

    #[test]
    fn streaks() {
        let today = d("2026-10-30");
        let days = |list: &[&str]| list.iter().map(|s| (d(s), 60_000)).collect::<Vec<_>>();
        // Today and the 3 days before.
        let s = summarize(
            &days(&["2026-10-27", "2026-10-28", "2026-10-29", "2026-10-30"]),
            today,
            0,
        );
        assert_eq!(s.streak_days, 4);
        // Nothing yet today: yesterday's streak still counts.
        let s = summarize(&days(&["2026-10-28", "2026-10-29"]), today, 0);
        assert_eq!(s.streak_days, 2);
        // Broken: last listened two days ago.
        let s = summarize(
            &days(&[
                "2026-10-27",
                "2026-10-28",
                "2026-10-01",
                "2026-10-02",
                "2026-10-03",
            ]),
            today,
            0,
        );
        assert_eq!((s.streak_days, s.best_streak_days), (0, 3));
        // Across a month boundary.
        let s = summarize(
            &days(&["2026-09-29", "2026-09-30", "2026-10-01"]),
            d("2026-10-01"),
            0,
        );
        assert_eq!(s.streak_days, 3);
        assert_eq!(summarize(&[], today, 0), Summary::default());
    }
}
