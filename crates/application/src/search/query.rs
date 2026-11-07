use hoarddeck_domain::{FileFormat, RelPath};

/// A search box input split into free words and filters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    pub words: Vec<String>,
    pub formats: Vec<FileFormat>,
    pub scope: Option<RelPath>,
}

/// Parses `dragon type:3mf in:"Miniatures/Wild Life"`. Unknown filters and invalid values are searched as words.
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
    fn unknown_filters_are_plain_words() {
        let parsed = parse_query("type:banana tag:mini in:../etc 10:30");
        assert_eq!(parsed.words, vec!["type:banana", "tag:mini", "in:../etc", "10:30"]);
        assert!(parsed.formats.is_empty() && parsed.scope.is_none());
    }
}
