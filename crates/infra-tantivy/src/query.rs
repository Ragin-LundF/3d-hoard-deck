use hoarddeck_application::search::SearchQuery;
use tantivy::Term;
use tantivy::query::{AllQuery, BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::tokenizer::{TextAnalyzer, TokenStream};

use crate::fields::{Fields, folder_facet, library_term_value, nfc};

/// Every word must match the title or the path, typo-tolerant and as a prefix; exact title words rank highest.
pub(crate) fn build(query: &SearchQuery, fields: Fields, analyzer: &mut TextAnalyzer) -> Box<dyn Query> {
    let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
    for word in query.words.iter().flat_map(|word| tokens(analyzer, word)) {
        clauses.push((Occur::Must, word_query(fields, &word)));
    }
    if let Some(library) = query.library {
        clauses.push((Occur::Must, term(Term::from_field_u64(fields.library, library_term_value(library)))));
        if let Some(scope) = &query.scope {
            clauses.push((Occur::Must, term(Term::from_facet(fields.folder, &folder_facet(library, scope)))));
        }
    }
    if !query.formats.is_empty() {
        let any_format = query
            .formats
            .iter()
            .map(|format| (Occur::Should, term(Term::from_field_text(fields.format, format.code()))))
            .collect();
        clauses.push((Occur::Must, Box::new(BooleanQuery::new(any_format))));
    }
    if clauses.is_empty() {
        return Box::new(AllQuery);
    }
    Box::new(BooleanQuery::new(clauses))
}

fn word_query(fields: Fields, word: &str) -> Box<dyn Query> {
    let distance = match word.chars().count() {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    };
    let title = Term::from_field_text(fields.title, word);
    let path = Term::from_field_text(fields.path, word);
    Box::new(BooleanQuery::new(vec![
        (Occur::Should, Box::new(BoostQuery::new(term(title.clone()), 4.0))),
        (Occur::Should, Box::new(BoostQuery::new(Box::new(FuzzyTermQuery::new_prefix(title, distance, true)), 2.0))),
        (Occur::Should, Box::new(FuzzyTermQuery::new_prefix(path, distance, true))),
    ]))
}

fn term(term: Term) -> Box<dyn Query> {
    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs))
}

fn tokens(analyzer: &mut TextAnalyzer, text: &str) -> Vec<String> {
    let text = nfc(text);
    let mut stream = analyzer.token_stream(&text);
    let mut result = Vec::new();
    while stream.advance() {
        result.push(stream.token().text.clone());
    }
    result
}
