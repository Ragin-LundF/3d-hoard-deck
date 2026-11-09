use super::{RuleKind, TagRule};

/// Suggested rules the user can add in Settings. Kept narrow on purpose: whole words only, scales limited to the usual
/// miniature sizes 28 to 75 mm (no `10mm` screw, no `120mm` fan), support words that step aside for "unsupported",
/// "resin" that skips printer parts, and a creator rule that skips sorting folders.
#[must_use]
pub fn tag_rule_presets() -> Vec<TagRule> {
    let rule = |tag: &str, kind: RuleKind, pattern: &str, unless: &str, name_only: bool| TagRule {
        tag: tag.to_owned(),
        kind,
        pattern: pattern.to_owned(),
        unless: unless.to_owned(),
        name_only,
        enabled: true,
    };
    vec![
        rule("{1}mm", RuleKind::Regex, r"\b(28|32|35|54|75) ?mm\b", "", false),
        rule(
            "presupported",
            RuleKind::Any,
            "presupported, supported, ready to slice",
            "unsupported, not supported, no supports, without supports",
            false,
        ),
        rule("unsupported", RuleKind::Any, "unsupported, no supports, without supports", "", false),
        rule("resin", RuleKind::Any, "resin", "printer, printers, vat, upgrade, upgrades", false),
        rule("fdm", RuleKind::Any, "fdm", "", false),
        rule("bust", RuleKind::Any, "bust, busts", "", true),
        rule("hollow", RuleKind::Any, "hollow, hollowed", "", true),
        rule("fantasy", RuleKind::Any, "fantasy", "", false),
        rule("sci-fi", RuleKind::Any, "sci-fi, science fiction", "", false),
        // Folders that sort instead of naming a creator; the count before saving shows what else to add here.
        rule("{1}", RuleKind::Folder, "Miniatures, MyMiniFactory", "collections, unsorted, busts", false),
    ]
}
