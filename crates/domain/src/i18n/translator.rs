//! Looks up messages in a Fluent catalog and fills in their arguments.

use std::fmt;
use std::sync::LazyLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use super::message_id::MessageId;

/// The English catalog, embedded at compile time.
const ENGLISH_LANGUAGE: &str = "en-US";
const ENGLISH_SOURCE: &str = include_str!("../../locales/en-US/app.ftl");

/// Built once, on first use. If the embedded catalog were invalid (the catalog
/// tests prevent that), every lookup would show its message ID instead of crashing.
static ENGLISH: LazyLock<Translator> = LazyLock::new(|| {
    Translator::from_ftl(ENGLISH_LANGUAGE, ENGLISH_SOURCE).unwrap_or_else(|_| Translator::empty())
});

/// A value substituted into a message, e.g. `{ $count }`.
///
/// Our own type, so callers don't depend on Fluent's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageArg {
    /// Text, inserted as-is.
    Text(String),
    /// A whole number; also selects plural forms (`[one]`, `[other]`).
    Integer(i64),
}

impl From<&str> for MessageArg {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for MessageArg {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<i64> for MessageArg {
    fn from(number: i64) -> Self {
        Self::Integer(number)
    }
}

/// Lets plain integer literals (`MessageArg::from(2)`) work without a suffix.
impl From<i32> for MessageArg {
    fn from(number: i32) -> Self {
        Self::Integer(i64::from(number))
    }
}

impl From<u32> for MessageArg {
    fn from(number: u32) -> Self {
        Self::Integer(i64::from(number))
    }
}

/// Why a Fluent catalog couldn't be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// The language tag (e.g. `en-US`) isn't valid.
    InvalidLanguage(String),
    /// The `.ftl` source has syntax errors; one description per error.
    Syntax(Vec<String>),
    /// The `.ftl` source defines the same message more than once.
    Duplicate(Vec<String>),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLanguage(tag) => write!(formatter, "invalid language tag {tag:?}"),
            Self::Syntax(errors) => {
                write!(formatter, "catalog syntax errors: {}", errors.join("; "))
            }
            Self::Duplicate(errors) => write!(
                formatter,
                "duplicate catalog entries: {}",
                errors.join("; ")
            ),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Turns [`MessageId`]s into text in one language.
pub struct Translator {
    bundle: FluentBundle<FluentResource>,
}

impl Translator {
    /// The built-in English translator, shared by the whole app.
    pub fn english() -> &'static Self {
        &ENGLISH
    }

    /// Builds a translator from Fluent source for one language.
    ///
    /// Bidi isolation marks are turned off: they're invisible characters around
    /// substituted values that only matter for right-to-left languages, and would
    /// otherwise appear in every string and test (decisions.md #022).
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError`] if the language tag is invalid, or the source has
    /// syntax errors or duplicate messages.
    pub fn from_ftl(language: &str, source: &str) -> Result<Self, CatalogError> {
        let language: LanguageIdentifier = language
            .parse()
            .map_err(|_| CatalogError::InvalidLanguage(language.to_owned()))?;
        let resource = FluentResource::try_new(source.to_owned()).map_err(|(_, errors)| {
            CatalogError::Syntax(errors.iter().map(ToString::to_string).collect())
        })?;

        let mut bundle = FluentBundle::new_concurrent(vec![language]);
        bundle.set_use_isolating(false);
        bundle.add_resource(resource).map_err(|errors| {
            CatalogError::Duplicate(errors.iter().map(ToString::to_string).collect())
        })?;
        Ok(Self { bundle })
    }

    /// A translator with no messages: every lookup shows the message ID.
    fn empty() -> Self {
        let mut bundle = FluentBundle::new_concurrent(Vec::new());
        bundle.set_use_isolating(false);
        Self { bundle }
    }

    /// The text of a message that takes no arguments.
    pub fn tr(&self, id: MessageId) -> String {
        self.format(id, None)
    }

    /// The text of a message with its arguments filled in, e.g.
    /// `tr_with(OPEN_SQUAWK_COUNT, &[("count", MessageArg::from(2))])`.
    pub fn tr_with(&self, id: MessageId, args: &[(&str, MessageArg)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            match value {
                MessageArg::Text(text) => fluent_args.set(*name, FluentValue::from(text.as_str())),
                MessageArg::Integer(number) => fluent_args.set(*name, FluentValue::from(*number)),
            }
        }
        self.format(id, Some(&fluent_args))
    }

    /// Whether the catalog has a message with this ID.
    pub fn has(&self, id: MessageId) -> bool {
        self.bundle.has_message(id.as_str())
    }

    /// Formats a message, falling back to its ID if it's missing or has no value,
    /// so a gap is visible on screen rather than a crash or a blank.
    fn format(&self, id: MessageId, args: Option<&FluentArgs<'_>>) -> String {
        let Some(pattern) = self
            .bundle
            .get_message(id.as_str())
            .and_then(|message| message.value())
        else {
            return id.as_str().to_owned();
        };
        // Formatting errors (e.g. a missing argument) still produce usable text,
        // with the problem shown inline as `{$name}`, so they're not surfaced here.
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::messages;

    /// Message IDs defined in Fluent source: lines like `nav-garage = Garage`.
    /// Comments, terms (`-brand`), and continuation lines are skipped.
    fn ftl_message_ids(source: &str) -> Vec<&str> {
        source
            .lines()
            .filter(|line| line.starts_with(|c: char| c.is_ascii_lowercase()))
            .filter_map(|line| line.split_once('='))
            .map(|(id, _)| id.trim())
            .collect()
    }

    #[test]
    fn english_catalog_parses_without_errors() {
        assert!(Translator::from_ftl(ENGLISH_LANGUAGE, ENGLISH_SOURCE).is_ok());
    }

    #[test]
    fn every_message_id_exists_in_english_catalog() {
        let english = Translator::english();
        let missing: Vec<&str> = messages::ALL
            .iter()
            .filter(|id| !english.has(**id))
            .map(|id| id.as_str())
            .collect();
        assert!(
            missing.is_empty(),
            "missing from en-US/app.ftl: {missing:?}"
        );
    }

    #[test]
    fn every_english_message_has_a_message_id() {
        let declared: Vec<&str> = messages::ALL.iter().map(|id| id.as_str()).collect();
        let undeclared: Vec<&str> = ftl_message_ids(ENGLISH_SOURCE)
            .into_iter()
            .filter(|id| !declared.contains(id))
            .collect();
        assert!(
            undeclared.is_empty(),
            "add to define_messages!: {undeclared:?}"
        );
    }

    #[test]
    fn tr_returns_english_text() {
        let english = Translator::english();
        assert_eq!(english.tr(messages::NAV_GARAGE), "Garage");
        assert_eq!(english.tr(messages::NAV_DUE_AND_SQUAWKS), "Due & squawks");
    }

    #[test]
    fn tr_with_substitutes_arguments_without_isolation_marks() {
        let translator = Translator::from_ftl("en-US", "greeting = Hello, { $name }!").unwrap();
        let greeting = MessageId::for_test("greeting");

        let text = translator.tr_with(greeting, &[("name", MessageArg::from("Sam"))]);

        assert_eq!(text, "Hello, Sam!");
        assert!(
            !text.contains(['\u{2068}', '\u{2069}']),
            "no bidi isolation marks"
        );
    }

    #[test]
    fn plural_message_picks_one_and_other_forms() {
        let english = Translator::english();
        let count = |n: i64| {
            english.tr_with(
                messages::OPEN_SQUAWK_COUNT,
                &[("count", MessageArg::from(n))],
            )
        };

        assert_eq!(count(1), "1 open squawk");
        assert_eq!(count(0), "0 open squawks");
        assert_eq!(count(3), "3 open squawks");
    }

    #[test]
    fn missing_message_falls_back_to_its_id() {
        let english = Translator::english();
        assert_eq!(
            english.tr(MessageId::for_test("not-a-real-message")),
            "not-a-real-message"
        );
    }

    #[test]
    fn invalid_catalogs_are_rejected_with_a_reason() {
        assert!(matches!(
            Translator::from_ftl("not a language", "a = b"),
            Err(CatalogError::InvalidLanguage(_))
        ));
        assert!(matches!(
            Translator::from_ftl("en-US", "broken = {"),
            Err(CatalogError::Syntax(_))
        ));
        assert!(matches!(
            Translator::from_ftl("en-US", "same = one\nsame = two"),
            Err(CatalogError::Duplicate(_))
        ));
    }
}
