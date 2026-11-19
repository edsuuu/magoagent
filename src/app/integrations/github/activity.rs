//! GitHub's contribution calendar, shown in Ajustes › Integrações and on the island's GitHub tab (Atividade), with the
//! person's repositories and their stars: one request every half hour, read by a plain function, away from the network.

use super::text_at;
use crate::prelude::*;

/// A day of the calendar.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Day {
    /// "2026-11-16"; empty when GitHub did not say.
    pub(crate) date: String,
    pub(crate) count: i64,
    /// 0 (none) to 4, GitHub's quartiles of the year.
    pub(crate) level: u8,
    /// 0 Sunday to 6 Saturday: its line in the calendar.
    pub(crate) weekday: u8,
}

/// The contribution calendar: the year's total and each week's days, the oldest week first.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Activity {
    pub(crate) total: i64,
    pub(crate) weeks: Vec<Vec<Day>>,
    /// Contributions in the last day of the calendar (today).
    pub(crate) today: i64,
    /// The person's own repositories, and the stars of the first 100 of them.
    pub(crate) repos: i64,
    pub(crate) stars: i64,
}

impl Activity {
    pub(crate) fn parse(root: &Value) -> Option<Activity> {
        let viewer = root.get("data")?.get("viewer")?;
        let calendar = viewer.get("contributionsCollection")?.get("contributionCalendar")?;
        let mut activity = Activity { total: calendar.get("totalContributions")?.as_i64()?.max(0), ..Default::default() };

        // a year is 53 weeks of 7 days: anything longer is not GitHub's answer
        for week in calendar.get("weeks")?.as_array()?.iter().take(60) {
            let Some(days) = week.get("contributionDays").and_then(Value::as_array) else { continue };

            let days: Vec<Day> = days
                .iter()
                .take(7)
                .enumerate()
                .map(|(index, day)| Day {
                    date: text_at(day, "date", 10).unwrap_or_default(),
                    count: day.get("contributionCount").and_then(Value::as_i64).unwrap_or(0).max(0),
                    level: match day.get("contributionLevel").and_then(Value::as_str) {
                        Some("FIRST_QUARTILE") => 1,
                        Some("SECOND_QUARTILE") => 2,
                        Some("THIRD_QUARTILE") => 3,
                        Some("FOURTH_QUARTILE") => 4,
                        _ => 0,
                    },
                    weekday: day.get("weekday").and_then(Value::as_u64).filter(|weekday| *weekday < 7).map_or(index as u8, |weekday| weekday as u8),
                })
                .collect();

            if let Some(last) = days.last() {
                activity.today = last.count;
                activity.weeks.push(days);
            }
        }

        if let Some(repositories) = viewer.get("repositories") {
            activity.repos = repositories.get("totalCount").and_then(Value::as_i64).unwrap_or(0).max(0);
            let stars = super::nodes(Some(repositories)).iter().filter_map(|node| node.get("stargazerCount").and_then(Value::as_i64));

            activity.stars = stars.map(|count| count.max(0)).sum();
        }

        Some(activity)
    }

    /// The last `count` days, the oldest first, each level graded against the busiest of them (the year's quartiles
    /// would leave a quiet week all grey).
    pub(crate) fn last_days(&self, count: usize) -> Vec<Day> {
        let all: Vec<&Day> = self.weeks.iter().flatten().collect();
        let days: Vec<Day> = all[all.len().saturating_sub(count)..].iter().map(|day| (*day).clone()).collect();
        let busiest = days.iter().map(|day| day.count).max().unwrap_or(0);

        if busiest == 0 {
            return days;
        }

        days.into_iter().map(|day| Day { level: if day.count == 0 { 0 } else { ((4 * day.count + busiest - 1) / busiest).clamp(1, 4) as u8 }, ..day }).collect()
    }

    /// "1.412 contribuições no último ano · 12 repositórios · ★ 1,3 mil".
    pub(crate) fn summary(&self) -> String {
        let mut parts = vec![match self.total {
            1 => "1 contribuição no último ano".to_string(),
            total => format!("{} contribuições no último ano", thousands(total)),
        }];

        match self.repos {
            0 => {}
            1 => parts.push("1 repositório".into()),
            repos => parts.push(format!("{repos} repositórios")),
        }

        if self.stars > 0 {
            parts.push(format!("★ {}", short_count(self.stars)));
        }

        parts.join(" · ")
    }
}

/// 1412 as "1.412".
fn thousands(number: i64) -> String {
    let digits = number.to_string();
    let mut out = String::new();

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('.');
        }

        out.push(digit);
    }

    out
}

/// 1340 as "1,3 mil" (a thousand and up), else as it is.
pub(crate) fn short_count(number: i64) -> String {
    if number < 1000 { number.to_string() } else { format!("{:.1} mil", number as f64 / 1000.0).replace('.', ",") }
}

/// A day in words: "16 de nov. · 22 contribuições" (the date as it came when it is not GitHub's).
pub(crate) fn day_words(day: &Day) -> String {
    const MONTHS: [&str; 12] = ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."];
    let mut parts = day.date.splitn(3, '-').skip(1).map(|part| part.parse::<usize>().ok());

    let date = match (parts.next().flatten(), parts.next().flatten()) {
        (Some(month @ 1..=12), Some(number @ 1..=31)) => format!("{number} de {}", MONTHS[month - 1]),
        _ => day.date.clone(),
    };

    let count = match day.count {
        0 => "nenhuma contribuição".to_string(),
        1 => "1 contribuição".to_string(),
        count => format!("{count} contribuições"),
    };

    [date, count].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// The calendar, and the person's repositories with their stars (the first 100: a person with more counts the stars of
/// the 100 pushed to last; one point of the hour's 5000 either way).
// ponytail: the stars of the first 100 repositories only, page through `repositories` if someone with more asks
pub(super) const ACTIVITY_QUERY: &str = r#"query {
  viewer {
    repositories(first: 100, ownerAffiliations: [OWNER], orderBy: {field: PUSHED_AT, direction: DESC}) { totalCount nodes { stargazerCount } }
    contributionsCollection { contributionCalendar {
      totalContributions
      weeks { contributionDays { date weekday contributionCount contributionLevel } }
    } }
  }
}"#;
