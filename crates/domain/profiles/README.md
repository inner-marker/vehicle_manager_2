# Asset profiles

One RON file per asset kind (decision #030), compiled into the app by `domain::profile`.
A profile is **data, not code**: what a kind starts with (meters, identifiers, custom
fields, event kinds, expected documents, and extra sections). A new asset gets a copy of
its profile's meters and fields, which it can then change.

- `id` is the kind's stable ID (it's stored on every asset): never change it.
- Bump `version` whenever a profile changes.
- Names are **message keys** in `crates/domain/locales/en-US/app.ftl`, never English text; a
  test checks every key exists.
- Kebab-case values that aren't plain RON identifiers are written as raw identifiers,
  e.g. `r#fuel-economy`.
- A meter `unit: None` means the person's distance preference (miles or kilometers).
- Each field has a stable `key` (e.g. `trip-time`, decision #057): formulas, reports, and voice
  refer to it, so never change one that has shipped.
