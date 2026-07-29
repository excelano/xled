//! Dates: the Excel-dialect format language, with parsing and rendering.
//!
//! Format tokens are Excel's (`YYYY`, `MM`, `DD`) rather than strftime's (`%Y`, `%m`, `%d`),
//! because the audience is the half-Excel user this whole tool is aimed at and nobody's Excel
//! memory holds `%Y`. Excel's own `mm`-means-minutes-after-an-hour wart cannot arise here:
//! xled has no time type, so a month token is never ambiguous.
//!
//! The load-bearing rule is that **xled never guesses DD/MM versus MM/DD**. A bare `date(x)`
//! reads ISO 8601 and nothing else; every other layout must be spelled out. `probe` exists
//! only to make the refusal useful — it reports which formats *would* have read a value, so
//! the error can name them. Nothing it finds is ever used as a result.
//!
//! Month and day names are English. A localized column is spelled out the same way any other
//! non-ISO layout is, by reformatting upstream or with `s///` first.

use chrono::{Datelike, NaiveDate};

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
// Monday first, matching the ISO weekday numbering that `weekday()` returns.
const DAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const DAY_ABBR: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Non-ISO layouts worth naming in an error, paired with the canonical spelling to suggest.
/// The probe formats use the 1-or-2-digit tokens so `3/4/2024` and `03/04/2024` both match,
/// while the suggestion shows the fixed-width form a user would most likely type.
const PROBES: &[(&str, &str)] = &[
    ("D/M/YYYY", "DD/MM/YYYY"),
    ("M/D/YYYY", "MM/DD/YYYY"),
    ("D-M-YYYY", "DD-MM-YYYY"),
    ("M-D-YYYY", "MM-DD-YYYY"),
    ("D.M.YYYY", "DD.MM.YYYY"),
    ("M.D.YYYY", "MM.DD.YYYY"),
    ("D/M/YY", "DD/MM/YY"),
    ("M/D/YY", "MM/DD/YY"),
    ("YYYY/M/D", "YYYY/MM/DD"),
    ("D MMM YYYY", "DD MMM YYYY"),
    ("MMM D, YYYY", "MMM DD, YYYY"),
    ("MMM D YYYY", "MMM DD YYYY"),
    ("MMMM D, YYYY", "MMMM DD, YYYY"),
    ("D MMMM YYYY", "DD MMMM YYYY"),
];

#[derive(Debug, PartialEq)]
enum Tok {
    Year4,
    Year2,
    MonthNum1,
    MonthNum2,
    MonthAbbr,
    MonthName,
    DayNum1,
    DayNum2,
    DayAbbr,
    DayName,
    Lit(char),
}

/// Split a format into tokens. A run of the same letter is one token, sized by its length
/// (`M` → 3, `MM` → 03, `MMM` → Mar, `MMMM` → March); everything else is a literal. A
/// backslash escapes the next character, so `\D` is a literal D rather than a day token.
///
/// Infallible by design: an unrecognized letter is simply a literal, which is what makes
/// `"D. MMMM YYYY"` work without a quoting rule.
fn tokenize(fmt: &str) -> Vec<Tok> {
    let c: Vec<char> = fmt.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\\' {
            i += 1;
            out.push(Tok::Lit(if i < c.len() { c[i] } else { '\\' }));
            i += 1;
            continue;
        }
        let lower = ch.to_ascii_lowercase();
        if matches!(lower, 'y' | 'm' | 'd') {
            let start = i;
            while i < c.len() && c[i].to_ascii_lowercase() == lower {
                i += 1;
            }
            let n = i - start;
            out.push(match (lower, n) {
                ('y', 1..=2) => Tok::Year2,
                ('y', _) => Tok::Year4,
                ('m', 1) => Tok::MonthNum1,
                ('m', 2) => Tok::MonthNum2,
                ('m', 3) => Tok::MonthAbbr,
                ('m', _) => Tok::MonthName,
                ('d', 1) => Tok::DayNum1,
                ('d', 2) => Tok::DayNum2,
                ('d', 3) => Tok::DayAbbr,
                (_, _) => Tok::DayName,
            });
            continue;
        }
        out.push(Tok::Lit(ch));
        i += 1;
    }
    out
}

/// Does this format name a year? A format that doesn't can render (`"MMMM"` for a month
/// label) but can never parse, since there is nothing to build a date from and inventing
/// one would be exactly the guess this module refuses to make.
pub fn has_year(fmt: &str) -> bool {
    tokenize(fmt)
        .iter()
        .any(|t| matches!(t, Tok::Year4 | Tok::Year2))
}

/// Render a date under an Excel format.
pub fn render(d: NaiveDate, fmt: &str) -> String {
    let mut out = String::new();
    let mon = d.month() as usize - 1;
    let dow = d.weekday().num_days_from_monday() as usize;
    for t in tokenize(fmt) {
        match t {
            Tok::Year4 => out.push_str(&format!("{:04}", d.year())),
            Tok::Year2 => out.push_str(&format!("{:02}", d.year().rem_euclid(100))),
            Tok::MonthNum1 => out.push_str(&d.month().to_string()),
            Tok::MonthNum2 => out.push_str(&format!("{:02}", d.month())),
            Tok::MonthAbbr => out.push_str(MONTH_ABBR[mon]),
            Tok::MonthName => out.push_str(MONTH_NAMES[mon]),
            Tok::DayNum1 => out.push_str(&d.day().to_string()),
            Tok::DayNum2 => out.push_str(&format!("{:02}", d.day())),
            Tok::DayAbbr => out.push_str(DAY_ABBR[dow]),
            Tok::DayName => out.push_str(DAY_NAMES[dow]),
            Tok::Lit(c) => out.push(c),
        }
    }
    out
}

/// Read a value under exactly one format. `None` means this value does not match — a data
/// outcome, never a program one.
///
/// The whole value must be consumed, so a trailing remainder is a mismatch rather than a
/// silent prefix read. A missing month or day defaults to the first (so `"MMM YYYY"` reads a
/// month column as its first day); a missing year cannot be defaulted, so it fails — callers
/// screen that with `has_year` and report it as the program error it is.
pub fn parse_with(s: &str, fmt: &str) -> Option<NaiveDate> {
    let c: Vec<char> = s.trim().chars().collect();
    let mut sc = Scan { c: &c, i: 0 };
    let (mut year, mut month, mut day) = (None, None, None);
    for t in tokenize(fmt) {
        match t {
            Tok::Year4 => year = Some(sc.digits(4, 4)? as i32),
            // Excel's pivot, and the reason YY is a last resort: 00–29 read as 2000s,
            // 30–99 as 1900s. Stated rather than discovered.
            Tok::Year2 => {
                let n = sc.digits(2, 2)?;
                year = Some(if n < 30 { 2000 + n } else { 1900 + n } as i32);
            }
            Tok::MonthNum1 => month = Some(sc.digits(1, 2)?),
            Tok::MonthNum2 => month = Some(sc.digits(2, 2)?),
            // Either width of name is accepted under either token: reading is unambiguous
            // even where writing is not, so "1 September 2024" matches "D MMM YYYY".
            Tok::MonthAbbr | Tok::MonthName => {
                month = Some(sc.word(&MONTH_ABBR, &MONTH_NAMES)? as u32 + 1)
            }
            Tok::DayNum1 => day = Some(sc.digits(1, 2)?),
            Tok::DayNum2 => day = Some(sc.digits(2, 2)?),
            // A weekday name carries no information the date doesn't — consume and discard,
            // so "Mon, 4 Mar 2024" reads without the caller stripping it first.
            Tok::DayAbbr | Tok::DayName => {
                sc.word(&DAY_ABBR, &DAY_NAMES)?;
            }
            Tok::Lit(ch) => sc.lit(ch)?,
        }
    }
    if sc.i != c.len() {
        return None;
    }
    NaiveDate::from_ymd_opt(year?, month.unwrap_or(1), day.unwrap_or(1))
}

/// Read ISO 8601: `2024-03-04`, its basic form `20240304`, or either followed by a time,
/// which is truncated to the date.
///
/// Truncating a timestamp is safe here precisely because the cast is explicit: the user asked
/// for a date, and there is only one date in `2024-03-04T14:03:00Z`. That is a different act
/// from picking between two readings of `03/04/2024`, which is why one is allowed and the
/// other is refused.
pub fn parse_iso(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    let head = match s.find(['T', 't', ' ']) {
        Some(i) => &s[..i],
        None => s,
    };
    if head.len() == 8 && head.bytes().all(|b| b.is_ascii_digit()) {
        return parse_with(head, "YYYYMMDD");
    }
    NaiveDate::parse_from_str(head, "%Y-%m-%d").ok()
}

/// Which known non-ISO layouts read this value, and to what. Used only to build an error
/// message: two entries disagreeing is the ambiguity that must be spelled out, one entry is a
/// format that must still be named, none means the value is not a date at all.
pub fn probe(s: &str) -> Vec<(&'static str, NaiveDate)> {
    PROBES
        .iter()
        .filter_map(|(fmt, shown)| parse_with(s, fmt).map(|d| (*shown, d)))
        .collect()
}

struct Scan<'a> {
    c: &'a [char],
    i: usize,
}

impl Scan<'_> {
    /// Take between `min` and `max` digits, consuming nothing on failure.
    fn digits(&mut self, min: usize, max: usize) -> Option<u32> {
        let start = self.i;
        while self.i < self.c.len() && self.i - start < max && self.c[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i - start < min {
            self.i = start;
            return None;
        }
        self.c[start..self.i].iter().collect::<String>().parse().ok()
    }

    /// Match a name from either candidate list, longest first so "June" is not read as "Jun"
    /// with a stray "e" left over. Returns the index within the lists, which is shared.
    fn word(&mut self, short: &[&str], long: &[&str]) -> Option<usize> {
        let mut best: Option<(usize, usize)> = None;
        for list in [short, long] {
            for (idx, w) in list.iter().enumerate() {
                let n = w.chars().count();
                if self.i + n <= self.c.len()
                    && self.c[self.i..self.i + n]
                        .iter()
                        .zip(w.chars())
                        .all(|(a, b)| a.eq_ignore_ascii_case(&b))
                    && best.is_none_or(|(_, bn)| n > bn)
                {
                    best = Some((idx, n));
                }
            }
        }
        let (idx, n) = best?;
        self.i += n;
        Some(idx)
    }

    /// Match one literal character. A whitespace literal absorbs a run of whitespace, so a
    /// double-spaced `"MMM  4, 2024"` still reads under `"MMM D, YYYY"`.
    fn lit(&mut self, ch: char) -> Option<()> {
        if ch.is_whitespace() {
            let start = self.i;
            while self.i < self.c.len() && self.c[self.i].is_whitespace() {
                self.i += 1;
            }
            return (self.i > start).then_some(());
        }
        if self.c.get(self.i) != Some(&ch) {
            return None;
        }
        self.i += 1;
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn tokens_are_sized_by_run_length() {
        assert_eq!(tokenize("YYYYMMDD").len(), 3);
        assert_eq!(tokenize("M"), vec![Tok::MonthNum1]);
        assert_eq!(tokenize("MMMM"), vec![Tok::MonthName]);
        // case-insensitive, and an unknown letter is just a literal
        assert_eq!(tokenize("yyyy"), vec![Tok::Year4]);
        assert_eq!(tokenize("Q"), vec![Tok::Lit('Q')]);
        // a backslash escapes a token letter back into a literal
        assert_eq!(tokenize(r"\D"), vec![Tok::Lit('D')]);
    }

    #[test]
    fn render_and_parse_round_trip() {
        let d = ymd(2024, 3, 4);
        for fmt in ["DD/MM/YYYY", "YYYY-MM-DD", "D MMM YYYY", "MMMM D, YYYY", "YYYYMMDD"] {
            let s = render(d, fmt);
            assert_eq!(parse_with(&s, fmt), Some(d), "round trip failed for {fmt}");
        }
    }

    #[test]
    fn render_uses_excel_widths() {
        let d = ymd(2024, 3, 4);
        assert_eq!(render(d, "DD/MM/YYYY"), "04/03/2024");
        assert_eq!(render(d, "D/M/YY"), "4/3/24");
        assert_eq!(render(d, "DDDD, D MMMM YYYY"), "Monday, 4 March 2024");
    }

    #[test]
    fn parse_requires_the_whole_value() {
        assert_eq!(parse_with("04/03/2024x", "DD/MM/YYYY"), None);
        assert_eq!(parse_with("04/03", "DD/MM/YYYY"), None);
    }

    #[test]
    fn parse_rejects_impossible_dates() {
        // the calendar check is chrono's, but it has to survive the token walk to reach it
        assert_eq!(parse_with("31/02/2024", "DD/MM/YYYY"), None);
        assert_eq!(parse_with("29/02/2024", "DD/MM/YYYY"), Some(ymd(2024, 2, 29)));
        assert_eq!(parse_with("29/02/2023", "DD/MM/YYYY"), None);
    }

    #[test]
    fn month_names_read_at_either_width() {
        assert_eq!(parse_with("1 September 2024", "D MMM YYYY"), Some(ymd(2024, 9, 1)));
        assert_eq!(parse_with("1 Sep 2024", "D MMMM YYYY"), Some(ymd(2024, 9, 1)));
        // "June" is not read as "Jun" with a leftover "e"
        assert_eq!(parse_with("1 June 2024", "D MMM YYYY"), Some(ymd(2024, 6, 1)));
    }

    #[test]
    fn weekday_names_are_consumed_and_ignored() {
        assert_eq!(parse_with("Mon, 4 Mar 2024", "DDD, D MMM YYYY"), Some(ymd(2024, 3, 4)));
    }

    #[test]
    fn a_missing_day_defaults_to_the_first() {
        assert_eq!(parse_with("Mar 2024", "MMM YYYY"), Some(ymd(2024, 3, 1)));
    }

    #[test]
    fn two_digit_years_use_excels_pivot() {
        assert_eq!(parse_with("01/01/29", "DD/MM/YY"), Some(ymd(2029, 1, 1)));
        assert_eq!(parse_with("01/01/30", "DD/MM/YY"), Some(ymd(1930, 1, 1)));
    }

    #[test]
    fn iso_covers_extended_basic_and_timestamps() {
        assert_eq!(parse_iso("2024-03-04"), Some(ymd(2024, 3, 4)));
        assert_eq!(parse_iso("20240304"), Some(ymd(2024, 3, 4)));
        assert_eq!(parse_iso("2024-03-04T14:03:00Z"), Some(ymd(2024, 3, 4)));
        assert_eq!(parse_iso("2024-03-04 14:03:00"), Some(ymd(2024, 3, 4)));
        assert_eq!(parse_iso("  2024-03-04  "), Some(ymd(2024, 3, 4)));
        // the layout xled refuses to guess is not ISO under any reading
        assert_eq!(parse_iso("03/04/2024"), None);
    }

    #[test]
    fn probe_separates_ambiguous_from_certain() {
        // both readings are real dates, and they differ — this is the halt case
        let hits = probe("03/04/2024");
        let dates: Vec<_> = hits.iter().map(|(_, d)| *d).collect();
        assert!(dates.contains(&ymd(2024, 4, 3)) && dates.contains(&ymd(2024, 3, 4)));

        // day 13 is no month, so only one reading survives — still not ISO, still named
        let hits = probe("13/04/2024");
        assert!(hits.iter().all(|(_, d)| *d == ymd(2024, 4, 13)));
        assert!(!hits.is_empty());

        // not a date in any known layout
        assert!(probe("banana").is_empty());
    }

    #[test]
    fn has_year_screens_unparseable_formats() {
        assert!(has_year("DD/MM/YYYY"));
        assert!(has_year("MM/YY"));
        assert!(!has_year("MMMM D"));
    }
}
