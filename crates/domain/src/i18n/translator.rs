//! Looks up messages in a Fluent catalog and fills in their arguments.
//!
//! **What the household calls squawks** (#064) is filled into every message: `{ $squawk }`,
//! `{ $squawks }`, `{ $Squawk }`, `{ $Squawks }`, and `{ $a-squawk }` ("an issue", "a
//! squawk"), from the chosen [`SquawkWord`]'s entry in the catalog (`squawk-word-issue` and
//! its attributes). [`Translator::tr`] and [`Translator::tr_with`] use the default word; the
//! app passes the household's to [`Translator::tr_for`] and [`Translator::tr_with_for`].

use std::fmt;
use std::sync::LazyLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use super::message_id::{MessageId, messages};
use crate::preferences::SquawkWord;

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

/// The argument names a squawk word fills in, in the order of [`SquawkForms`].
const SQUAWK_ARGS: [&str; 5] = ["squawk", "squawks", "Squawk", "Squawks", "a-squawk"];

/// The catalog attributes holding each form (the entry's own value is the first).
const SQUAWK_ATTRIBUTES: [&str; 4] = ["plural", "title", "title-plural", "article"];

/// One word's forms: singular, plural, their capitalized forms, and with its article.
type SquawkForms = [String; 5];

/// Turns [`MessageId`]s into text in one language.
pub struct Translator {
    bundle: FluentBundle<FluentResource>,
    /// Each [`SquawkWord`]'s forms, in [`SquawkWord::ALL`]'s order.
    squawk_forms: Vec<SquawkForms>,
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
        let mut translator = Self {
            bundle,
            squawk_forms: Vec::new(),
        };
        translator.squawk_forms = SquawkWord::ALL
            .iter()
            .map(|word| translator.forms_of(*word))
            .collect();
        Ok(translator)
    }

    /// A translator with no messages: every lookup shows the message ID.
    fn empty() -> Self {
        let mut bundle = FluentBundle::new_concurrent(Vec::new());
        bundle.set_use_isolating(false);
        Self {
            bundle,
            squawk_forms: Vec::new(),
        }
    }

    /// The catalog entry naming `word`.
    const fn word_entry(word: SquawkWord) -> MessageId {
        match word {
            SquawkWord::Issue => messages::SQUAWK_WORD_ISSUE,
            SquawkWord::Squawk => messages::SQUAWK_WORD_SQUAWK,
            SquawkWord::Fault => messages::SQUAWK_WORD_FAULT,
            SquawkWord::WriteUp => messages::SQUAWK_WORD_WRITE_UP,
        }
    }

    /// `word`'s forms from the catalog (a missing one shows its entry's ID and attribute).
    fn forms_of(&self, word: SquawkWord) -> SquawkForms {
        let key = Self::word_entry(word).as_str();
        let message = self.bundle.get_message(key);
        let attribute = |name: &str| {
            message
                .as_ref()
                .and_then(|message| message.get_attribute(name))
                .map(|attribute| attribute.value())
        };
        let patterns = [
            message
                .as_ref()
                .and_then(fluent_bundle::FluentMessage::value),
            attribute(SQUAWK_ATTRIBUTES[0]),
            attribute(SQUAWK_ATTRIBUTES[1]),
            attribute(SQUAWK_ATTRIBUTES[2]),
            attribute(SQUAWK_ATTRIBUTES[3]),
        ];
        let mut errors = Vec::new();
        std::array::from_fn(|index| {
            patterns[index].map_or_else(
                || format!("{key}.{}", SQUAWK_ARGS[index]),
                |pattern| {
                    self.bundle
                        .format_pattern(pattern, None, &mut errors)
                        .into_owned()
                },
            )
        })
    }

    /// The forms of the word to fill in.
    fn squawk_forms(&self, word: SquawkWord) -> Option<&SquawkForms> {
        let index = SquawkWord::ALL.iter().position(|each| *each == word)?;
        self.squawk_forms.get(index)
    }

    /// The text of a message that takes no arguments (squawks by their default word).
    pub fn tr(&self, id: MessageId) -> String {
        self.tr_for(SquawkWord::default(), id)
    }

    /// The text of a message with its arguments filled in, e.g.
    /// `tr_with(OPEN_SQUAWK_COUNT, &[("count", MessageArg::from(2))])` (squawks by their
    /// default word).
    pub fn tr_with(&self, id: MessageId, args: &[(&str, MessageArg)]) -> String {
        self.tr_with_for(SquawkWord::default(), id, args)
    }

    /// The text of a message that takes no arguments, with squawks called `word`.
    pub fn tr_for(&self, word: SquawkWord, id: MessageId) -> String {
        self.tr_with_for(word, id, &[])
    }

    /// The text of a message with its arguments filled in, and squawks called `word`.
    pub fn tr_with_for(
        &self,
        word: SquawkWord,
        id: MessageId,
        args: &[(&str, MessageArg)],
    ) -> String {
        let mut fluent_args = FluentArgs::new();
        if let Some(forms) = self.squawk_forms(word) {
            for (name, form) in SQUAWK_ARGS.iter().zip(forms) {
                fluent_args.set(*name, FluentValue::from(form.as_str()));
            }
        }
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

    /// The text of a message named by data rather than code (e.g. a profile's meter name in
    /// its RON file), so it can't be a [`MessageId`]. A test checks every such key exists;
    /// a missing one shows as the key itself.
    pub fn tr_key(&self, key: &str) -> String {
        let Some(pattern) = self
            .bundle
            .get_message(key)
            .and_then(|message| message.value())
        else {
            return key.to_owned();
        };
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, None, &mut errors)
            .into_owned()
    }

    /// Whether the catalog has a message with this key (see [`Translator::tr_key`]).
    pub fn has_key(&self, key: &str) -> bool {
        self.bundle.has_message(key)
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
        assert_eq!(english.tr(messages::NAV_DUE_AND_SQUAWKS), "Due & issues");
    }

    #[test]
    fn squawks_are_called_by_the_households_word() {
        let english = Translator::english();
        let count = [("count", MessageArg::from(1))];
        let cases = [
            (SquawkWord::Issue, "Due & issues", "1 open issue", "Issues"),
            (
                SquawkWord::Squawk,
                "Due & squawks",
                "1 open squawk",
                "Squawks",
            ),
            (SquawkWord::Fault, "Due & faults", "1 open fault", "Faults"),
            (
                SquawkWord::WriteUp,
                "Due & write-ups",
                "1 open write-up",
                "Write-ups",
            ),
        ];
        for (word, nav, one, tab) in cases {
            assert_eq!(english.tr_for(word, messages::NAV_DUE_AND_SQUAWKS), nav);
            assert_eq!(
                english.tr_with_for(word, messages::OPEN_SQUAWK_COUNT, &count),
                one
            );
            assert_eq!(english.tr_for(word, messages::TAB_SQUAWKS), tab);
        }
        let forms = english.squawk_forms(SquawkWord::Issue).unwrap();
        assert_eq!(forms[4], "an issue");
        assert_eq!(
            english.squawk_forms(SquawkWord::Squawk).unwrap()[4],
            "a squawk"
        );
    }

    #[test]
    fn no_message_says_squawk_unless_thats_the_word() {
        let english = Translator::english();
        let saying: Vec<&str> = messages::ALL
            .iter()
            .filter(|id| **id != messages::SQUAWK_WORD_SQUAWK)
            .filter(|id| {
                english
                    .tr_for(SquawkWord::Issue, **id)
                    .to_lowercase()
                    .contains("squawk")
            })
            .map(|id| id.as_str())
            .collect();
        assert!(
            saying.is_empty(),
            "write squawk through {{ $squawk }} and its friends: {saying:?}"
        );
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

        assert_eq!(count(1), "1 open issue");
        assert_eq!(count(0), "0 open issues");
        assert_eq!(count(3), "3 open issues");
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
