use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct YearMonth {
    pub year: u16,
    pub month: u8,
}

impl YearMonth {
    /// Parses `YYYY-MM`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (year, month) = text.split_once('-')?;
        if year.len() != 4 || month.len() != 2 {
            return None;
        }
        let year: u16 = year.parse().ok()?;
        let month: u8 = month.parse().ok()?;
        ((1990..=2100).contains(&year) && (1..=12).contains(&month)).then_some(Self { year, month })
    }
}

impl fmt::Display for YearMonth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    pub display: String,
    pub released: Option<YearMonth>,
}

/// `2019-05 - Ogre Set 1` → "Ogre Set 1" released 2019-05; `Alien+Xenomorph - 4543820` → "Alien Xenomorph".
#[must_use]
pub fn parse_folder_name(name: &str) -> ParsedName {
    let released = name.get(..7).and_then(YearMonth::parse);
    let rest = match released {
        Some(_) => skip_day(&name[7..]).trim_start_matches([' ', '-', '_']),
        None => name,
    };
    let display = clean(rest);
    if display.is_empty() {
        return ParsedName { display: clean(name), released: None };
    }
    ParsedName { display, released }
}

/// Title for a single file: extensions (also disguised ones like `.stl.txt`) removed, then cleaned.
#[must_use]
pub fn file_title(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let stem_len = lower.find(".stl.").unwrap_or_else(|| lower.rfind('.').filter(|&i| i > 0).unwrap_or(lower.len()));
    clean(&name[..stem_len])
}

/// `-12 Event` after `2019-05` is a full date; the day is dropped with the prefix.
fn skip_day(rest: &str) -> &str {
    let bytes = rest.as_bytes();
    let is_day = bytes.len() >= 3 && bytes[0] == b'-' && bytes[1].is_ascii_digit() && bytes[2].is_ascii_digit();
    let ends_there = bytes.get(3).is_none_or(|next| !next.is_ascii_digit());
    if is_day && ends_there { &rest[3..] } else { rest }
}

fn clean(text: &str) -> String {
    let spaced = text.replace(['+', '_'], " ");
    let without_id = strip_trailing_id(&spaced);
    without_id.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Removes download-site ids like ` - 4543820` at the end.
fn strip_trailing_id(text: &str) -> &str {
    let trimmed = text.trim_end();
    let digits = trimmed.len() - trimmed.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits < 5 {
        return text;
    }
    let before = trimmed[..trimmed.len() - digits].trim_end();
    match before.strip_suffix('-') {
        Some(head) if !head.trim().is_empty() => head,
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_prefix_becomes_date_and_display_name() {
        let parsed = parse_folder_name("2019-05 - Ogre Set 1");
        assert_eq!(parsed.display, "Ogre Set 1");
        assert_eq!(parsed.released, Some(YearMonth { year: 2019, month: 5 }));
        assert_eq!(parsed.released.map(|r| r.to_string()), Some("2019-05".to_owned()));
    }

    #[test]
    fn names_are_cleaned_but_meaningful_numbers_stay() {
        assert_eq!(parse_folder_name("Alien+Xenomorph - 4543820").display, "Alien Xenomorph");
        assert_eq!(parse_folder_name("Space_Marine  75mm").display, "Space Marine 75mm");
        assert_eq!(parse_folder_name("Set 2023").display, "Set 2023");
        assert_eq!(parse_folder_name("2022-13 - Not a date").released, None);
        assert_eq!(parse_folder_name("2022-09").display, "2022-09");
        assert_eq!(parse_folder_name("2019-05-12 Event").display, "Event");
        assert_eq!(parse_folder_name("2019-05-123 Set").display, "123 Set");
    }

    #[test]
    fn file_titles_drop_disguised_extensions() {
        assert_eq!(file_title("Dragon_Head.stl.txt"), "Dragon Head");
        assert_eq!(file_title("13661.3mf"), "13661");
        assert_eq!(file_title(".hidden"), ".hidden");
    }
}
