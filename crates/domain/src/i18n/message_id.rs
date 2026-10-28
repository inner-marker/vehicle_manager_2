//! Typed message IDs, so code can only refer to messages that are declared.

/// The ID of one message in the Fluent catalog, e.g. `nav-garage`.
///
/// Only the constants in [`messages`] exist, so a mistyped ID is a compile error,
/// and a test checks every constant has an English message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(&'static str);

impl MessageId {
    /// The message's ID as written in the `.ftl` file.
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// An ID outside [`messages`], for tests of undeclared or ad-hoc catalogs.
    #[cfg(test)]
    pub(crate) const fn for_test(id: &'static str) -> Self {
        Self(id)
    }
}

/// Declares each message constant and the [`messages::ALL`] list in one place,
/// so a constant can't be added without being covered by the catalog test.
macro_rules! define_messages {
    ($($(#[$doc:meta])* $name:ident => $id:literal,)+) => {
        /// Every message the app can show. Names mirror the `.ftl` IDs.
        pub mod messages {
            use super::MessageId;

            $(
                $(#[$doc])*
                pub const $name: MessageId = MessageId($id);
            )+

            /// Every declared message, in declaration order.
            pub const ALL: &[MessageId] = &[$($name),+];
        }
    };
}

define_messages! {
    /// Navigation: the home screen with one card per vehicle.
    NAV_GARAGE => "nav-garage",
    /// Navigation: maintenance coming due, and open squawks.
    NAV_DUE_AND_SQUAWKS => "nav-due-and-squawks",
    /// Navigation: the in-app message center.
    NAV_MESSAGES => "nav-messages",
    /// Navigation: every event across vehicles, newest first.
    NAV_TIMELINE => "nav-timeline",
    /// Navigation: history reports and exports.
    NAV_REPORTS => "nav-reports",
    /// Navigation: managing vehicles.
    NAV_VEHICLES => "nav-vehicles",
    /// Navigation: app settings.
    NAV_SETTINGS => "nav-settings",
    /// Navigation: the debug log viewer and export.
    NAV_DIAGNOSTICS => "nav-diagnostics",
    /// "N open squawks"; takes `count` (number).
    OPEN_SQUAWK_COUNT => "open-squawk-count",
}
