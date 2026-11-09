use hoarddeck_domain::{FileFormat, RelPath};

/// A search box input split into free words and filters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    pub words: Vec<String>,
    pub formats: Vec<FileFormat>,
    /// Lower-case tag names from `tag:` filters; every one must be present.
    pub tags: Vec<String>,
    /// From `tag:a|b`: at least one of them must be present.
    pub any_tags: Vec<String>,
    /// From `-tag:` filters: none of them may be present.
    pub excluded_tags: Vec<String>,
    pub favorite: bool,
    pub printed: bool,
    pub scope: Option<RelPath>,
}

/// Parses `dragon type:3mf tag:resin -tag:bust tag:32mm|28mm is:favorite in:"Miniatures/Wild Life"`. Unknown
/// filters and invalid values are searched as words.
#[must_use]
pub fn parse_query(input: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    for token in tokens(input) {
        let filter = token.split_once(':').filter(|(key, value)| !key.is_empty() && !value.is_empty());
        match filter {
            Some(("type" | "format", value)) => match FileFormat::from_code(&value.to_ascii_lowercase()) {
                Ok(format) => parsed.formats.push(format),
                Err(_) => parsed.words.push(token),
            },
            Some(("tag", value)) if value.contains('|') => {
                parsed.any_tags.extend(value.split('|').filter(|tag| !tag.is_empty()).map(str::to_lowercase));
            }
            Some(("tag", value)) => parsed.tags.push(value.to_lowercase()),
            Some(("-tag", value)) => parsed.excluded_tags.push(value.to_lowercase()),
            Some(("is", "favorite" | "favourite" | "fav")) => parsed.favorite = true,
            Some(("is", "printed")) => parsed.printed = true,
            Some(("in", value)) => match RelPath::new(value.trim_matches('/')) {
                Ok(scope) => parsed.scope = Some(scope),
                Err(_) => parsed.words.push(token),
            },
            _ => parsed.words.push(token),
        }
    }
    parsed
}

/// Whitespace-separated tokens; double quotes keep spaces, e.g. `in:"Wild Life"`.
fn tokens(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in input.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words_and_filters() {
        let parsed = parse_query(r#"red dragon type:3MF in:"Miniatures/Wild Life""#);
        assert_eq!(parsed.words, vec!["red", "dragon"]);
        assert_eq!(parsed.formats, vec![FileFormat::ThreeMf]);
        assert_eq!(parsed.scope, Some(RelPath::new("Miniatures/Wild Life").expect("valid path")));
    }

    #[test]
    fn tags_can_be_excluded_or_alternatives() {
        let parsed = parse_query(r#"-tag:bust tag:"32mm|28 mm" tag:resin"#);
        assert_eq!(parsed.excluded_tags, vec!["bust"]);
        assert_eq!(parsed.any_tags, vec!["32mm", "28 mm"]);
        assert_eq!(parsed.tags, vec!["resin"]);
        assert_eq!(parsed.words, Vec::<String>::new());
    }

    #[test]
    fn tags_and_marks_are_filters() {
        let parsed = parse_query(r#"tag:"Sci Fi" tag:Resin is:favorite is:printed knight"#);
        assert_eq!(parsed.tags, vec!["sci fi", "resin"]);
        assert!(parsed.favorite && parsed.printed);
        assert_eq!(parsed.words, vec!["knight"]);
    }

    #[test]
    fn unknown_filters_are_plain_words() {
        let parsed = parse_query("type:banana is:sleepy in:../etc 10:30");
        assert_eq!(parsed.words, vec!["type:banana", "is:sleepy", "in:../etc", "10:30"]);
        assert!(parsed.formats.is_empty() && parsed.scope.is_none());
    }
}
