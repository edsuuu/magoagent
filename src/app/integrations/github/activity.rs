//! GitHub's contribution calendar, shown in Ajustes › Integrações: one request every half hour, read by a plain
//! function, away from the network.

use crate::prelude::*;

/// The contribution calendar: the year's total and each week's days as levels 0 to 4, the oldest week first.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Activity {
    pub(crate) total: i64,
    pub(crate) weeks: Vec<Vec<u8>>,
    /// Contributions in the last day of the calendar (today).
    pub(crate) today: i64,
}

impl Activity {
    pub(crate) fn parse(root: &Value) -> Option<Activity> {
        let calendar = root.get("data")?.get("viewer")?.get("contributionsCollection")?.get("contributionCalendar")?;
        let mut activity = Activity { total: calendar.get("totalContributions")?.as_i64()?.max(0), ..Default::default() };
        // a year is 53 weeks of 7 days: anything longer is not GitHub's answer
        for week in calendar.get("weeks")?.as_array()?.iter().take(60) {
            let Some(days) = week.get("contributionDays").and_then(Value::as_array) else { continue };
            let levels: Vec<u8> = days
                .iter()
                .take(7)
                .map(|day| match day.get("contributionLevel").and_then(Value::as_str) {
                    Some("FIRST_QUARTILE") => 1,
                    Some("SECOND_QUARTILE") => 2,
                    Some("THIRD_QUARTILE") => 3,
                    Some("FOURTH_QUARTILE") => 4,
                    _ => 0,
                })
                .collect();
            if let Some(count) = days.iter().take(7).next_back().and_then(|day| day.get("contributionCount")).and_then(Value::as_i64) {
                activity.today = count.max(0);
            }
            if !levels.is_empty() {
                activity.weeks.push(levels);
            }
        }
        Some(activity)
    }
}

pub(super) const ACTIVITY_QUERY: &str = r#"query {
  viewer { contributionsCollection { contributionCalendar {
    totalContributions
    weeks { contributionDays { contributionCount contributionLevel } }
  } } }
}"#;
