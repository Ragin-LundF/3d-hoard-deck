//! Tiny helpers over quick-xml: element names and attributes without namespace prefixes.

use quick_xml::events::BytesStart;

pub(crate) fn name(element: &BytesStart<'_>) -> String {
    element.local_name().as_ref().to_owned()
}

/// Attribute value by local name (`p:path` matches `path`). Values are taken as written: 3MF numbers and paths
/// carry no XML entities in practice.
pub(crate) fn attribute(element: &BytesStart<'_>, wanted: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == wanted)
        .map(|attribute| attribute.value.into_owned())
}

pub(crate) fn number<T: std::str::FromStr>(element: &BytesStart<'_>, wanted: &str) -> Option<T> {
    attribute(element, wanted)?.trim().parse().ok()
}
