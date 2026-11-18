//! Pure parsing and date maths for the AI Developer Cockpit (no Win32), so it
//! can be unit-tested anywhere.

use serde_json::Value;

/// Provider the cockpit knows how to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAi,
    DeepSeek,
}

impl Provider {
    pub const ALL: [Provider; 3] = [Provider::Anthropic, Provider::OpenAi, Provider::DeepSeek];

    /// Name used for the Credential Manager entry and on the CLI.
    pub fn key(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAi => "openai",
            Provider::DeepSeek => "deepseek",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Claude",
            Provider::OpenAi => "OpenAI",
            Provider::DeepSeek => "DeepSeek",
        }
    }

    pub fn parse(name: &str) -> Option<Provider> {
        match name.to_ascii_lowercase().as_str() {
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "openai" | "gpt" => Some(Provider::OpenAi),
            "deepseek" => Some(Provider::DeepSeek),
            _ => None,
        }
    }
}

/// One provider's month-to-date figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stat {
    pub configured: bool,
    /// Month-to-date spend in USD.
    pub cost: Option<f64>,
    /// Month-to-date tokens.
    pub tokens: Option<u64>,
    /// Prepaid balance, e.g. "$18.40" (DeepSeek).
    pub balance: Option<String>,
    /// Daily spend in USD, index 0 = day 1 of the month.
    pub daily: Vec<f64>,
    pub error: Option<String>,
}

// ---- dates (UTC, proleptic Gregorian) ----

/// Days since 1970-01-01 -> (year, month, day). Howard Hinnant's algorithm.
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

/// (year, month, day) -> days since 1970-01-01.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    (days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)) as u32
}

/// The current month as seen from `now_unix`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Month {
    pub year: i64,
    pub month: u32,
    /// Today's day of month (1-based).
    pub today: u32,
    pub length: u32,
    /// Unix seconds of the 1st at 00:00 UTC.
    pub start_unix: i64,
}

impl Month {
    pub fn at(now_unix: i64) -> Month {
        let days = now_unix.div_euclid(86_400);
        let (year, month, today) = civil(days);
        Month {
            year,
            month,
            today,
            length: days_in_month(year, month),
            start_unix: days_from_civil(year, month, 1) * 86_400,
        }
    }

    pub fn start_rfc3339(&self) -> String {
        format!("{:04}-{:02}-01T00:00:00Z", self.year, self.month)
    }

    /// Tomorrow 00:00 UTC (exclusive end that includes today).
    pub fn end_rfc3339(&self) -> String {
        let (y, m, d) = civil(days_from_civil(self.year, self.month, self.today) + 1);
        format!("{y:04}-{m:02}-{d:02}T00:00:00Z")
    }

    pub fn days_left(&self) -> u32 {
        self.length - self.today
    }
}

/// "2026-11-03T00:00:00Z" -> 3 when it falls in `month`.
fn day_of(stamp: &str, month: &Month) -> Option<usize> {
    let y: i64 = stamp.get(0..4)?.parse().ok()?;
    let m: u32 = stamp.get(5..7)?.parse().ok()?;
    let d: usize = stamp.get(8..10)?.parse().ok()?;
    (y == month.year && m == month.month).then_some(d)
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

// ---- Anthropic: /v1/organizations/analytics/{cost,usage} ----

/// Daily USD from an `/analytics/cost` page (`cost.amount` is in cents).
pub fn anthropic_cost(body: &str, month: &Month) -> Option<Vec<f64>> {
    let value: Value = serde_json::from_str(body).ok()?;
    let mut daily = vec![0.0; month.today as usize];
    for row in value.get("data")?.as_array()? {
        let Some(day) = row.get("starting_at").and_then(Value::as_str).and_then(|s| day_of(s, month)) else {
            continue;
        };
        let cents = row.get("cost").and_then(|c| c.get("amount")).and_then(number).unwrap_or(0.0);
        if let Some(slot) = daily.get_mut(day - 1) {
            *slot += cents / 100.0;
        }
    }
    Some(daily)
}

/// Total tokens from an `/analytics/usage` page.
pub fn anthropic_tokens(body: &str) -> Option<u64> {
    let value: Value = serde_json::from_str(body).ok()?;
    Some(
        value
            .get("data")?
            .as_array()?
            .iter()
            .filter_map(|row| row.get("usage")?.get("total_tokens")?.as_u64())
            .sum(),
    )
}

// ---- OpenAI: /v1/organization/{costs,usage/completions} ----

/// Daily USD from an OpenAI costs page (`results[].amount.value`, dollars).
pub fn openai_cost(body: &str, month: &Month) -> Option<Vec<f64>> {
    let value: Value = serde_json::from_str(body).ok()?;
    let mut daily = vec![0.0; month.today as usize];
    for bucket in value.get("data")?.as_array()? {
        let start = bucket.get("start_time").and_then(Value::as_i64).unwrap_or(0);
        let index = (start - month.start_unix).div_euclid(86_400);
        if index < 0 {
            continue;
        }
        let dollars: f64 = bucket
            .get("results")
            .and_then(Value::as_array)
            .map(|results| {
                results
                    .iter()
                    .filter_map(|r| r.get("amount")?.get("value").and_then(number))
                    .sum()
            })
            .unwrap_or(0.0);
        if let Some(slot) = daily.get_mut(index as usize) {
            *slot += dollars;
        }
    }
    Some(daily)
}

/// Input + output tokens from an OpenAI completions usage page.
pub fn openai_tokens(body: &str) -> Option<u64> {
    let value: Value = serde_json::from_str(body).ok()?;
    let mut total = 0u64;
    for bucket in value.get("data")?.as_array()? {
        for r in bucket.get("results").and_then(Value::as_array).into_iter().flatten() {
            total += r.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
            total += r.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
        }
    }
    Some(total)
}

// ---- DeepSeek: /user/balance ----

pub fn deepseek_balance(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let info = value.get("balance_infos")?.as_array()?.first()?;
    let amount = info.get("total_balance").and_then(number)?;
    let currency = info.get("currency").and_then(Value::as_str).unwrap_or("USD");
    Some(match currency {
        "USD" => format!("${amount:.2}"),
        "CNY" => format!("¥{amount:.2}"),
        other => format!("{amount:.2} {other}"),
    })
}

/// 32_400 -> "32.4k", 1_500_000 -> "1.5M".
pub fn compact(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1e3),
        1_000_000..=999_999_999 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.1}B", n as f64 / 1e9),
    }
}

/// Short reason for a non-2xx reply.
pub fn status_error(status: u32) -> String {
    match status {
        401 | 403 => "key rejected (needs admin key)".into(),
        404 => "endpoint not available".into(),
        429 => "rate limited".into(),
        s => format!("HTTP {s}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2026, 11, 18), 20_775);
        assert_eq!(civil(20_775), (2026, 11, 18));
        assert_eq!(days_in_month(2028, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        let m = Month::at(20_775 * 86_400 + 3600);
        assert_eq!((m.year, m.month, m.today, m.length), (2026, 11, 18, 30));
        assert_eq!(m.start_rfc3339(), "2026-11-01T00:00:00Z");
        assert_eq!(m.end_rfc3339(), "2026-11-19T00:00:00Z");
        assert_eq!(m.days_left(), 12);
        let dec = Month::at(days_from_civil(2026, 12, 31) * 86_400);
        assert_eq!(dec.end_rfc3339(), "2027-01-01T00:00:00Z");
    }

    #[test]
    fn anthropic() {
        let m = Month::at(days_from_civil(2026, 11, 3) * 86_400);
        let body = r#"{"data":[
            {"starting_at":"2026-11-01T00:00:00Z","ending_at":"2026-11-02T00:00:00Z","group":{},"cost":{"currency":"USD","amount":"1250.5","list_amount":"1300"}},
            {"starting_at":"2026-11-03T00:00:00Z","ending_at":"2026-11-04T00:00:00Z","group":{},"cost":{"currency":"USD","amount":"200","list_amount":"200"}}
        ],"has_more":false,"next_page":null,"data_refreshed_at":"2026-11-03T10:00:00Z"}"#;
        let daily = anthropic_cost(body, &m).unwrap();
        assert_eq!(daily.len(), 3);
        assert!((daily[0] - 12.505).abs() < 1e-9);
        assert_eq!(daily[1], 0.0);
        assert!((daily[2] - 2.0).abs() < 1e-9);
        let usage = r#"{"data":[{"starting_at":"2026-11-01T00:00:00Z","usage":{"total_tokens":1000}},{"usage":{"total_tokens":234}}]}"#;
        assert_eq!(anthropic_tokens(usage), Some(1234));
    }

    #[test]
    fn openai() {
        let m = Month::at(days_from_civil(2026, 11, 2) * 86_400);
        let s = m.start_unix;
        let body = format!(
            r#"{{"object":"page","data":[
              {{"object":"bucket","start_time":{s},"end_time":{e},"results":[{{"object":"organization.costs.result","amount":{{"value":3.5,"currency":"usd"}}}},{{"amount":{{"value":0.25,"currency":"usd"}}}}]}},
              {{"object":"bucket","start_time":{t},"end_time":{u},"results":[{{"amount":{{"value":1.0,"currency":"usd"}}}}]}}
            ],"has_more":false}}"#,
            e = s + 86_400,
            t = s + 86_400,
            u = s + 2 * 86_400
        );
        assert_eq!(openai_cost(&body, &m).unwrap(), vec![3.75, 1.0]);
        let usage = r#"{"data":[{"results":[{"input_tokens":100,"output_tokens":50}]},{"results":[]}]}"#;
        assert_eq!(openai_tokens(usage), Some(150));
    }

    #[test]
    fn misc() {
        let body = r#"{"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"18.40","granted_balance":"0","topped_up_balance":"18.40"}]}"#;
        assert_eq!(deepseek_balance(body).as_deref(), Some("$18.40"));
        assert_eq!(compact(32_400), "32.4k");
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_250_000), "1.2M");
        assert_eq!(Provider::parse("Claude"), Some(Provider::Anthropic));
    }
}
