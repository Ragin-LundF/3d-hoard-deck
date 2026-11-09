use std::collections::HashMap;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use hoarddeck_domain::{Model, RelPath, TagName};

use crate::AppError;

const MAX_RULES: usize = 200;
const MAX_PATTERN: usize = 1_000;
/// Keeps a pasted monster regex from eating memory; the regex crate itself runs in linear time.
const REGEX_SIZE_LIMIT: usize = 1 << 20;
const CAPTURE: &str = "{1}";
/// Words or phrases per rule; each is one bit of a `u64`.
const MAX_TERMS: usize = 64;

/// How a rule looks at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleKind {
    /// Any of the comma-separated words or phrases.
    Any,
    /// All of them, in the same path.
    All,
    /// A regular expression on the path's words; `{1}` in the tag is replaced by the first capture.
    Regex,
    /// The folder right below one of the comma-separated folder names becomes the tag (`{1}`), e.g. a creator.
    Folder,
}

/// One tag rule as the user edits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRule {
    pub tag: String,
    pub kind: RuleKind,
    pub pattern: String,
    /// Comma-separated words or phrases; a path containing any of them gets nothing from this rule.
    #[serde(default)]
    pub unless: String,
    /// Look only at the model's own name and the files below it, not at the folders above it.
    #[serde(default)]
    pub name_only: bool,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

/// Rules ready to run. Words are matched whole: a name is split into words at spaces, punctuation, `_`, `-`,
/// CamelCase and digit boundaries, and a word or phrase matches only whole consecutive words of one folder or file
/// name (`ready to slice` matches `ReadyToSlice` and `Ready_to_Slice`, `bust` does not match `Robust`). A regex also
/// looks at one name at a time, its words joined by single spaces (`32mm` is `32 mm`).
#[derive(Debug, Default)]
pub struct CompiledTagRules {
    rules: Vec<Compiled>,
}

#[derive(Debug)]
struct Compiled {
    tag: String,
    matcher: Matcher,
    unless: Vec<String>,
    name_only: bool,
    enabled: bool,
}

#[derive(Debug)]
enum Matcher {
    Any(Vec<String>),
    All(Vec<String>),
    Regex(Regex),
    Folder(Vec<String>),
}

/// What one rule found in one name, or in several names together.
#[derive(Debug, Clone, Copy, Default)]
struct Hit {
    unless: bool,
    /// One bit per word or phrase of the rule that the name contains.
    terms: u64,
    /// The regex's first group, when it matched, as an index into [`Evaluator::captures`].
    capture: Option<usize>,
}

impl Hit {
    fn add(&mut self, other: Self) {
        self.unless |= other.unless;
        self.terms |= other.terms;
        self.capture = self.capture.or(other.capture);
    }
}

/// A folder or file name, looked at once per regroup however many paths contain it.
struct Name {
    /// All its letters and digits in lower case, to compare with folder names of folder rules.
    letters: String,
    hits: Vec<Hit>,
}

/// Looks at models with a cache of names; one per regroup or count.
struct Evaluator<'a> {
    rules: &'a [Compiled],
    names: HashMap<&'a str, Name>,
    captures: Vec<String>,
}

impl<'a> Evaluator<'a> {
    fn new(rules: &'a [Compiled]) -> Self {
        Self { rules, names: HashMap::new(), captures: Vec::new() }
    }

    fn name(&mut self, name: &'a str) -> &Name {
        let (rules, captures) = (self.rules, &mut self.captures);
        self.names.entry(name).or_insert_with(|| {
            let (text, ranges) = words_of(name);
            let words: Vec<&str> = ranges.iter().map(|&(start, end)| &text[start..end]).collect();
            let hits = rules.iter().map(|rule| rule.hit(&words, &text, captures)).collect();
            Name { letters: words.concat(), hits }
        })
    }

    /// What each rule found in these names together.
    fn hits(&mut self, names: impl Iterator<Item = &'a str>) -> Vec<Hit> {
        let mut total = vec![Hit::default(); self.rules.len()];
        for name in names {
            for (sum, hit) in total.iter_mut().zip(&self.name(name).hits) {
                sum.add(*hit);
            }
        }
        total
    }

    /// The tag of each rule for one model, or `None`.
    fn model(&mut self, model: &'a Model) -> Vec<Option<TagName>> {
        let names: Vec<&'a str> = model.path.segments().collect();
        let below = names.len().saturating_sub(1);
        let whole = self.hits(names.iter().copied());
        let own = self.hits(names[below..].iter().copied());
        let files: Vec<Vec<Hit>> = model
            .files
            .iter()
            .filter(|file| file.kind.is_model_file() && file.path != model.path)
            .map(|file| self.hits(file.path.segments().skip(names.len())))
            .collect();
        let rules = self.rules;
        rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                if let Matcher::Folder(folders) = &rule.matcher {
                    return self.folder_tag(rule, folders, &names, &whole[index]);
                }
                let start = if rule.name_only { own[index] } else { whole[index] };
                // Each path on its own: a model with `Supported` and `Unsupported` variant folders gets both tags.
                std::iter::once(start)
                    .chain(files.iter().map(|file| {
                        let mut path = start;
                        path.add(file[index]);
                        path
                    }))
                    .find_map(|path| rule.tag_of(path, &self.captures))
            })
            .collect()
    }

    /// The name below the deepest listed folder, but never the model's own folder or file.
    fn folder_tag(&mut self, rule: &Compiled, folders: &[String], names: &[&'a str], whole: &Hit) -> Option<TagName> {
        if whole.unless {
            return None;
        }
        let mut found = None;
        for (index, name) in names.iter().enumerate() {
            if folders.contains(&self.name(name).letters) {
                found = Some(index + 1);
            }
        }
        let below = found?;
        if below + 1 >= names.len() {
            return None;
        }
        TagName::new(&rule.tag.replace(CAPTURE, names[below])).ok()
    }
}

impl CompiledTagRules {
    /// # Errors
    /// Returns [`AppError::InvalidInput`] naming the first invalid rule.
    pub fn compile(rules: &[TagRule]) -> Result<Self, AppError> {
        if rules.len() > MAX_RULES {
            return Err(AppError::InvalidInput(format!("at most {MAX_RULES} tag rules are possible")));
        }
        let rules = rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                compile_rule(rule).map_err(|reason| {
                    AppError::InvalidInput(format!("tag rule {} ({}): {reason}", index + 1, rule.tag.trim()))
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { rules })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.rules.iter().any(|rule| rule.enabled)
    }

    /// The tags the switched-on rules give each model, in rule order, each once.
    #[must_use]
    pub fn tag_models(&self, models: &[Model]) -> Vec<Vec<TagName>> {
        let mut evaluator = Evaluator::new(&self.rules);
        models
            .iter()
            .map(|model| {
                let found = evaluator.model(model);
                self.switched_on(found)
            })
            .collect()
    }

    /// For each rule, switched on or not, how many of `models` it tags. Used to count matches before saving.
    #[must_use]
    pub fn count_matches(&self, models: &[Model]) -> Vec<u32> {
        let mut evaluator = Evaluator::new(&self.rules);
        let mut counts = vec![0_u32; self.rules.len()];
        for model in models {
            for (count, tag) in counts.iter_mut().zip(evaluator.model(model)) {
                *count += u32::from(tag.is_some());
            }
        }
        counts
    }

    /// The tags for a typed path, as if it were a model file, for the test field in Settings.
    #[must_use]
    pub fn tags_for_path(&self, path: &RelPath) -> Vec<String> {
        let model = Model {
            path: path.clone(),
            folder: path.parent().unwrap_or_default(),
            kind: hoarddeck_domain::ModelKind::File,
            title: String::new(),
            released: None,
            cover: None,
            collection: None,
            marked_by_user: false,
            files: Vec::new(),
            auto_tags: Vec::new(),
        };
        let found = Evaluator::new(&self.rules).model(&model);
        self.switched_on(found).into_iter().map(String::from).collect()
    }

    fn switched_on(&self, found: Vec<Option<TagName>>) -> Vec<TagName> {
        let mut tags: Vec<TagName> = Vec::new();
        for (rule, tag) in self.rules.iter().zip(found) {
            if let Some(tag) = tag.filter(|_| rule.enabled)
                && !tags.iter().any(|known| known.key() == tag.key())
            {
                tags.push(tag);
            }
        }
        tags
    }
}

impl Compiled {
    fn hit(&self, words: &[&str], text: &str, captures: &mut Vec<String>) -> Hit {
        let bits = |terms: &[String]| {
            terms.iter().enumerate().filter(|(_, term)| contains(words, term)).fold(0, |bits, (bit, _)| bits | 1 << bit)
        };
        let unless = self.unless.iter().any(|term| contains(words, term));
        match &self.matcher {
            Matcher::Any(terms) | Matcher::All(terms) => Hit { unless, terms: bits(terms), capture: None },
            Matcher::Regex(regex) => Hit {
                unless,
                terms: 0,
                // `is_match` first: most names do not match, and it is much faster than finding groups.
                capture: regex.is_match(text).then(|| regex.captures(text)).flatten().map(|found| {
                    captures.push(found.get(1).map_or_else(String::new, |group| group.as_str().to_owned()));
                    captures.len() - 1
                }),
            },
            Matcher::Folder(_) => Hit { unless, ..Hit::default() },
        }
    }

    /// The tag for one path from what its names hold together.
    fn tag_of(&self, path: Hit, captures: &[String]) -> Option<TagName> {
        if path.unless {
            return None;
        }
        let tag = match &self.matcher {
            Matcher::Any(_) => (path.terms != 0).then(|| self.tag.clone())?,
            Matcher::All(terms) => (path.terms.count_ones() as usize == terms.len()).then(|| self.tag.clone())?,
            Matcher::Regex(_) => self.tag.replace(CAPTURE, captures.get(path.capture?)?),
            Matcher::Folder(_) => return None,
        };
        TagName::new(&tag).ok()
    }
}

/// A term matches whole consecutive words of one name whose letters together are the term's letters.
fn contains(words: &[&str], term: &str) -> bool {
    (0..words.len()).any(|start| {
        let mut rest = term;
        for word in &words[start..] {
            match rest.strip_prefix(word) {
                Some("") => return true,
                Some(tail) => rest = tail,
                None => return false,
            }
        }
        false
    })
}

fn compile_rule(rule: &TagRule) -> Result<Compiled, String> {
    let tag = rule.tag.trim().to_owned();
    if rule.pattern.len() > MAX_PATTERN || rule.unless.len() > MAX_PATTERN {
        return Err("the pattern is too long".to_owned());
    }
    let matcher = match rule.kind {
        RuleKind::Any => Matcher::Any(terms(&rule.pattern)?),
        RuleKind::All => Matcher::All(terms(&rule.pattern)?),
        RuleKind::Folder => Matcher::Folder(terms(&rule.pattern)?),
        RuleKind::Regex => {
            let regex = RegexBuilder::new(rule.pattern.trim())
                .case_insensitive(true)
                .size_limit(REGEX_SIZE_LIMIT)
                .build()
                .map_err(|_| "the regular expression is not valid".to_owned())?;
            if tag.contains(CAPTURE) && regex.captures_len() < 2 {
                return Err("the tag uses {1}, but the expression has no group in ( )".to_owned());
            }
            Matcher::Regex(regex)
        }
    };
    let uses_capture = matches!(matcher, Matcher::Regex(_) | Matcher::Folder(_));
    if !uses_capture && tag.contains(CAPTURE) {
        return Err("{1} can only be used with a regular expression or a folder rule".to_owned());
    }
    if TagName::new(&tag.replace(CAPTURE, "x")).is_err() {
        return Err("the tag needs 1 to 64 characters".to_owned());
    }
    let unless = if rule.unless.trim().is_empty() { Vec::new() } else { terms(&rule.unless)? };
    Ok(Compiled { tag, matcher, unless, name_only: rule.name_only, enabled: rule.enabled })
}

/// Comma-separated words or phrases, each reduced to its lower-case letters and digits.
fn terms(text: &str) -> Result<Vec<String>, String> {
    let terms: Vec<String> =
        text.split(',').map(|term| split_words(term).concat()).filter(|term| !term.is_empty()).collect();
    if terms.is_empty() {
        Err("add at least one word".to_owned())
    } else if terms.len() > MAX_TERMS {
        Err(format!("at most {MAX_TERMS} words or phrases"))
    } else {
        Ok(terms)
    }
}

/// Lower-case words of one name: split at anything but letters and digits, at lower-to-upper (`UnSupported`), at the
/// end of a run of capitals (`STLFlix`) and between letters and digits (`32mm`).
fn split_words(name: &str) -> Vec<String> {
    let (text, words) = words_of(name);
    words.into_iter().map(|(start, end)| text[start..end].to_owned()).collect()
}

/// The words of a name joined by single spaces, and where each word is in that text.
fn words_of(name: &str) -> (String, Vec<(usize, usize)>) {
    let mut text = String::with_capacity(name.len() + 8);
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut previous: Option<char> = None;
    let mut chars = name.chars().peekable();
    while let Some(char) = chars.next() {
        if !char.is_alphanumeric() {
            if let Some(begin) = start.take() {
                words.push((begin, text.len()));
            }
            previous = None;
            continue;
        }
        if let (Some(before), Some(begin)) = (previous, start) {
            let next_is_lower = chars.peek().is_some_and(|next| next.is_lowercase());
            let boundary = (before.is_lowercase() && char.is_uppercase())
                || (before.is_uppercase() && char.is_uppercase() && next_is_lower)
                || (before.is_numeric() != char.is_numeric());
            if boundary {
                words.push((begin, text.len()));
                start = None;
            }
        }
        if start.is_none() {
            if !text.is_empty() {
                text.push(' ');
            }
            start = Some(text.len());
        }
        text.extend(char.to_lowercase());
        previous = Some(char);
    }
    if let Some(begin) = start {
        words.push((begin, text.len()));
    }
    (text, words)
}

#[cfg(test)]
mod tests {
    use hoarddeck_domain::{FileFormat, FileKind, ModelFile, ModelKind};

    use super::*;
    use crate::tagging::tag_rule_presets;

    fn presets() -> CompiledTagRules {
        CompiledTagRules::compile(&tag_rule_presets()).expect("presets compile")
    }

    fn tags(path: &str) -> Vec<String> {
        presets().tags_for_path(&RelPath::new(path).expect("valid"))
    }

    fn names_of(rules: &CompiledTagRules, model: &Model) -> Vec<String> {
        rules.tag_models(std::slice::from_ref(model)).remove(0).into_iter().map(String::from).collect()
    }

    fn folder_model(path: &str, files: &[&str]) -> Model {
        let path = RelPath::new(path).expect("valid");
        Model {
            folder: path.parent().unwrap_or_default(),
            files: files
                .iter()
                .map(|file| ModelFile {
                    path: RelPath::new(format!("{path}/{file}")).expect("valid"),
                    kind: if file.to_lowercase().ends_with(".png") {
                        FileKind::Image
                    } else {
                        FileKind::Mesh(FileFormat::Stl)
                    },
                    size: 1,
                    variant: None,
                })
                .collect(),
            kind: ModelKind::Folder,
            title: path.name().to_owned(),
            released: None,
            cover: None,
            collection: None,
            marked_by_user: false,
            auto_tags: Vec::new(),
            path,
        }
    }

    #[test]
    fn names_are_split_into_words() {
        assert_eq!(split_words("OniMage_32mm_UnSupported"), ["oni", "mage", "32", "mm", "un", "supported"]);
        assert_eq!(split_words("STLFlix ReadytoSlice"), ["stl", "flix", "readyto", "slice"]);
        assert_eq!(split_words("Sci-Fi Über"), ["sci", "fi", "über"]);
    }

    #[test]
    fn real_library_paths_get_the_expected_tags() {
        assert_eq!(
            tags("Miniatures/Loot Studios/Fantasy/2023-05 - Tales of Ryuboken/resin/32mm/OniMage_32mm_UnSupported"),
            ["32mm", "unsupported", "resin", "fantasy", "Loot Studios"]
        );
        assert_eq!(
            tags(
                "Miniatures/Loot Studios/SciFi/2024-11 - Cryo Hazard/resin/75mm/PitblazerUltraWarden_75mm_ReadytoSlice"
            ),
            ["75mm", "presupported", "resin", "sci-fi", "Loot Studios"]
        );
        assert_eq!(
            tags("Miniatures/MyMiniFactory/Animal Den Miniatures/2026-08 - Animal Collector/Jaguar swimming"),
            ["Animal Den Miniatures"]
        );
    }

    #[test]
    fn unrelated_words_give_no_tags() {
        // Parts of words, numbers that are no miniature scale, and the model's own folder as "creator".
        assert_eq!(tags("Props/Robust Crate/Superfantasyland/Resinous Tree"), Vec::<String>::new());
        assert_eq!(tags("Hardware/M3 screws 10mm/Fan 40mm/Nozzle 0.4mm/Year 2032mm"), Vec::<String>::new());
        assert_eq!(tags("Miniatures/Dragon"), Vec::<String>::new());
        assert_eq!(tags("Miniatures/hollowknight.stl"), Vec::<String>::new());
        assert!(tags("Terrain/Unsupported walls").iter().all(|tag| tag != "presupported"));
        // A bust in a folder above the model does not make every model in it a bust.
        assert!(!tags("Busts/Knight/Sword").contains(&"bust".to_owned()));
        assert!(tags("Heroes/Knight bust").contains(&"bust".to_owned()));
    }

    #[test]
    fn variant_folders_count_one_by_one() {
        let model = folder_model("Dragon", &["Supported/dragon.stl", "Unsupported/dragon.stl", "pre supported.png"]);
        let names = names_of(&presets(), &model);
        assert_eq!(names, ["presupported", "unsupported"]);
        let only_unsupported = folder_model("Dragon", &["Unsupported/dragon.stl"]);
        let names = names_of(&presets(), &only_unsupported);
        assert_eq!(names, ["unsupported"], "\"supported\" inside UnSupported does not count, and images are ignored");
    }

    #[test]
    fn rules_can_be_switched_off_and_counted() {
        let mut rules = tag_rule_presets();
        for rule in &mut rules {
            rule.enabled = rule.tag == "resin";
        }
        let compiled = CompiledTagRules::compile(&rules).expect("valid");
        let model = folder_model("Resin/32mm/Knight", &["knight.stl"]);
        assert_eq!(names_of(&compiled, &model), ["resin"]);
        assert_eq!(
            compiled.count_matches(std::slice::from_ref(&model)).iter().sum::<u32>(),
            2,
            "32mm counts while off"
        );
    }

    #[test]
    fn invalid_rules_are_refused_with_their_number() {
        let rule = |tag: &str, kind: RuleKind, pattern: &str| TagRule {
            tag: tag.to_owned(),
            kind,
            pattern: pattern.to_owned(),
            unless: String::new(),
            name_only: false,
            enabled: true,
        };
        let refused = |rules: &[TagRule]| CompiledTagRules::compile(rules).expect_err("refused").to_string();
        assert!(refused(&[rule("a", RuleKind::Any, "a"), rule("x", RuleKind::Regex, "(")]).contains("tag rule 2"));
        assert!(refused(&[rule("{1}", RuleKind::Regex, r"\d+")]).contains("no group"));
        assert!(refused(&[rule("{1}", RuleKind::Any, "dragon")]).contains("{1}"));
        assert!(refused(&[rule("dragon", RuleKind::Any, " , ")]).contains("at least one word"));
        assert!(refused(&[rule(" ", RuleKind::Any, "dragon")]).contains("1 to 64"));
    }
}
