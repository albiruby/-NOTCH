//! Quick math and currency for Ask: "12*(3+4)=" or "100usd to idr=".
//! Math is evaluated locally; currency asks Frankfurter (ECB rates, no key).

/// Evaluate an arithmetic expression: + - * / ^ %, parentheses, decimals.
/// `x`, `×`, `÷` and `:` are accepted as operators; `,` as a decimal point.
pub fn eval(input: &str) -> Option<f64> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            'x' | 'X' | '×' => '*',
            '÷' | ':' => '/',
            ',' => '.',
            other => other,
        })
        .collect();
    let mut p = Parser { s: cleaned.as_bytes(), i: 0 };
    let value = p.expr()?;
    (p.i == p.s.len() && value.is_finite()).then_some(value)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat(b'+') {
                v += self.term()?;
            } else if self.eat(b'-') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
        }
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        loop {
            if self.eat(b'*') {
                v *= self.power()?;
            } else if self.eat(b'/') {
                v /= self.power()?;
            } else {
                return Some(v);
            }
        }
    }
    fn power(&mut self) -> Option<f64> {
        let base = self.unary()?;
        if self.eat(b'^') {
            Some(base.powf(self.power()?))
        } else {
            Some(base)
        }
    }
    fn unary(&mut self) -> Option<f64> {
        if self.eat(b'-') {
            return Some(-self.unary()?);
        }
        if self.eat(b'+') {
            return self.unary();
        }
        let mut v = self.primary()?;
        if self.eat(b'%') {
            v /= 100.0;
        }
        Some(v)
    }
    fn primary(&mut self) -> Option<f64> {
        if self.eat(b'(') {
            let v = self.expr()?;
            return self.eat(b')').then_some(v);
        }
        let start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9' | b'.')) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()
    }
}

/// "100usd to idr", "100 USD in IDR", "50 eur ke idr" → (100, "USD", "IDR").
pub fn parse_currency(input: &str) -> Option<(f64, String, String)> {
    let lower = input.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    // Allow "100usd" as one word.
    let mut tokens: Vec<String> = Vec::new();
    for w in words {
        let split = w.find(|c: char| c.is_ascii_alphabetic()).filter(|&i| i > 0);
        match split {
            Some(i) => {
                tokens.push(w[..i].to_string());
                tokens.push(w[i..].to_string());
            }
            None => tokens.push(w.to_string()),
        }
    }
    let [amount, from, joiner, to] = tokens.as_slice() else {
        return None;
    };
    if !matches!(joiner.as_str(), "to" | "in" | "ke" | "=>" | "->") {
        return None;
    }
    let code = |s: &str| (s.len() == 3 && s.chars().all(|c| c.is_ascii_alphabetic())).then(|| s.to_uppercase());
    Some((eval(amount)?, code(from)?, code(to)?))
}

/// Exchange rate from → to, from Frankfurter (ECB reference rates).
pub fn rate(from: &str, to: &str) -> Option<f64> {
    let path = format!("/latest?from={from}&to={to}");
    let (status, body) = crate::http::get("api.frankfurter.app", &path, &[])?;
    if !(200..300).contains(&status) {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    value.get("rates")?.get(to)?.as_f64()
}

/// Group thousands and trim trailing zeros: 1234567.5 → "1,234,567.5".
pub fn format_number(v: f64) -> String {
    let rounded = (v * 1e6).round() / 1e6;
    let negative = rounded < 0.0;
    let text = format!("{:.6}", rounded.abs());
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let frac = frac.trim_end_matches('0');
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let sign = if negative { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math() {
        assert_eq!(eval("12*(3+4)"), Some(84.0));
        assert_eq!(eval("2^10"), Some(1024.0));
        assert_eq!(eval("15% * 200"), Some(30.0));
        assert_eq!(eval("10 x 3 : 2"), Some(15.0));
        assert_eq!(eval("1,5 + 1"), Some(2.5));
        assert_eq!(eval("-(2+3)"), Some(-5.0));
        assert_eq!(eval("2+"), None);
        assert_eq!(eval("1/0"), None);
    }

    #[test]
    fn currency_parsing() {
        assert_eq!(parse_currency("100usd to idr"), Some((100.0, "USD".into(), "IDR".into())));
        assert_eq!(parse_currency("50 EUR in JPY"), Some((50.0, "EUR".into(), "JPY".into())));
        assert_eq!(parse_currency("hello world"), None);
    }

    #[test]
    fn numbers() {
        assert_eq!(format_number(1234567.5), "1,234,567.5");
        assert_eq!(format_number(84.0), "84");
        assert_eq!(format_number(-1000.0), "-1,000");
    }
}
