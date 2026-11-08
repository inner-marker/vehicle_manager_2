//! Display preferences against a real database, through the command pipeline.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod preferences {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{PreferencesOutcome, PreferencesView};
    use domain::preferences::{
        Choice, CurrencyCode, DateFormat, DistanceUnit, EconomyUnit, GarageSort, IdleSignOut,
        Language, Originals, PreferencesPatch, PressureUnit, SquawkWord, VolumeUnit,
    };
    use domain::test_support::{ManualClock, test_id};
    use server::health::Health;
    use server::preferences::{
        PreferencesContext, set_household, set_mine, view as preferences_view,
    };
    use server::sessions::SignedIn;
    use storage::Database;

    fn context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> PreferencesContext<'a> {
        PreferencesContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            clock,
        }
    }

    fn unwrap_view(outcome: PreferencesOutcome) -> PreferencesView {
        let PreferencesOutcome::Preferences { view } = outcome else {
            panic!("expected preferences, got {outcome:?}");
        };
        view
    }

    #[tokio::test]
    async fn mine_beats_the_household_which_beats_the_built_in_default() {
        let (_temp, database, mut stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        stuart.mode = ActiveMode::Admin;

        let household = PreferencesPatch {
            volume: Some(Choice::Set(VolumeUnit::Liters)),
            distance: Some(Choice::Set(DistanceUnit::Kilometers)),
            ..PreferencesPatch::default()
        };
        unwrap_view(set_household(&context(&database, &stuart, &clock), household).await);
        let mine = PreferencesPatch {
            distance: Some(Choice::Set(DistanceUnit::Miles)),
            ..PreferencesPatch::default()
        };
        let view = unwrap_view(set_mine(&context(&database, &sam, &clock), mine).await);

        assert_eq!(view.effective.distance, DistanceUnit::Miles, "Sam's own");
        assert_eq!(view.effective.volume, VolumeUnit::Liters, "the household's");
        assert_eq!(view.effective.pressure, PressureUnit::Psi, "built in");
        assert_eq!(view.household.distance, Some(DistanceUnit::Kilometers));
    }

    #[tokio::test]
    async fn using_the_household_default_again_clears_my_choice() {
        let (_temp, database, _stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let ctx = context(&database, &sam, &clock);

        unwrap_view(
            set_mine(
                &ctx,
                PreferencesPatch {
                    pressure: Some(Choice::Set(PressureUnit::Bar)),
                    ..PreferencesPatch::default()
                },
            )
            .await,
        );
        let view = unwrap_view(
            set_mine(
                &ctx,
                PreferencesPatch {
                    pressure: Some(Choice::Default),
                    ..PreferencesPatch::default()
                },
            )
            .await,
        );

        assert_eq!(view.mine.pressure, None);
        assert_eq!(view.effective.pressure, PressureUnit::Psi);
    }

    #[tokio::test]
    async fn household_defaults_need_the_admin_in_admin_mode() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let patch = PreferencesPatch {
            distance: Some(Choice::Set(DistanceUnit::Kilometers)),
            ..PreferencesPatch::default()
        };

        assert_eq!(
            set_household(&context(&database, &sam, &clock), patch).await,
            PreferencesOutcome::NotAllowed
        );
        assert_eq!(
            set_household(&context(&database, &stuart, &clock), patch).await,
            PreferencesOutcome::NotAllowed,
            "Driver mode"
        );
    }

    #[tokio::test]
    async fn every_choice_of_every_setting_is_stored_and_read_back() {
        let (_temp, database, _stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let ctx = context(&database, &sam, &clock);
        // Each row sets one value of every setting; together they cover every choice.
        let rows = [
            (
                DistanceUnit::Miles,
                VolumeUnit::UsGallons,
                EconomyUnit::MpgUs,
                PressureUnit::Psi,
                DateFormat::DayMonthNameYear,
            ),
            (
                DistanceUnit::Kilometers,
                VolumeUnit::ImperialGallons,
                EconomyUnit::MpgUk,
                PressureUnit::Kpa,
                DateFormat::MonthDayYear,
            ),
            (
                DistanceUnit::Kilometers,
                VolumeUnit::Liters,
                EconomyUnit::LitersPer100Km,
                PressureUnit::Bar,
                DateFormat::DayMonthYear,
            ),
            (
                DistanceUnit::Miles,
                VolumeUnit::Liters,
                EconomyUnit::KmPerLiter,
                PressureUnit::Bar,
                DateFormat::YearMonthDay,
            ),
        ];
        for (distance, volume, economy, pressure, date_format) in rows {
            let patch = PreferencesPatch {
                distance: Some(Choice::Set(distance)),
                volume: Some(Choice::Set(volume)),
                economy: Some(Choice::Set(economy)),
                pressure: Some(Choice::Set(pressure)),
                date_format: Some(Choice::Set(date_format)),
                currency: Some(Choice::Set(CurrencyCode::parse("CAD").unwrap())),
                language: Some(Choice::Set(Language::EnglishUs)),
                idle_sign_out: None,
                garage_sort: None,
                squawk_word: None,
                originals: None,
            };
            let view = unwrap_view(set_mine(&ctx, patch).await);
            assert_eq!(
                (
                    view.mine.distance,
                    view.mine.volume,
                    view.mine.economy,
                    view.mine.pressure,
                    view.mine.date_format
                ),
                (
                    Some(distance),
                    Some(volume),
                    Some(economy),
                    Some(pressure),
                    Some(date_format)
                )
            );
            assert_eq!(view.mine.currency.unwrap().as_str(), "CAD");
        }
    }

    #[tokio::test]
    async fn every_sign_out_and_garage_choice_is_stored_and_read_back() {
        let (_temp, database, _stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let ctx = context(&database, &sam, &clock);
        for idle_sign_out in [
            IdleSignOut::OneHour,
            IdleSignOut::FourHours,
            IdleSignOut::EightHours,
            IdleSignOut::TwentyFourHours,
            IdleSignOut::Never,
        ] {
            let patch = PreferencesPatch {
                idle_sign_out: Some(Choice::Set(idle_sign_out)),
                ..PreferencesPatch::default()
            };
            let view = unwrap_view(set_mine(&ctx, patch).await);
            assert_eq!(view.mine.idle_sign_out, Some(idle_sign_out));
            assert_eq!(view.effective.idle_sign_out, idle_sign_out);
        }
        for garage_sort in [
            GarageSort::NewestActivity,
            GarageSort::Name,
            GarageSort::Custom,
        ] {
            let patch = PreferencesPatch {
                garage_sort: Some(Choice::Set(garage_sort)),
                ..PreferencesPatch::default()
            };
            let view = unwrap_view(set_mine(&ctx, patch).await);
            assert_eq!(view.effective.garage_sort, garage_sort);
        }
    }

    #[tokio::test]
    async fn what_squawks_are_called_is_the_admins_choice_for_everyone() {
        let (_temp, database, mut stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let word = |word| PreferencesPatch {
            squawk_word: Some(Choice::Set(word)),
            ..PreferencesPatch::default()
        };
        assert_eq!(
            set_mine(&context(&database, &sam, &clock), word(SquawkWord::Fault)).await,
            PreferencesOutcome::NotAllowed,
            "a person can't choose their own"
        );
        let view = unwrap_view(preferences_view(&context(&database, &sam, &clock)).await);
        assert_eq!(view.effective.squawk_word, SquawkWord::Issue, "the default");

        stuart.mode = ActiveMode::Admin;
        for chosen in SquawkWord::ALL {
            unwrap_view(set_household(&context(&database, &stuart, &clock), word(chosen)).await);
            let sams = unwrap_view(preferences_view(&context(&database, &sam, &clock)).await);
            assert_eq!(sams.effective.squawk_word, chosen);
        }
    }

    #[tokio::test]
    async fn keeping_originals_is_the_households_switch() {
        let (_temp, database, mut stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let originals = |choice| PreferencesPatch {
            originals: Some(Choice::Set(choice)),
            ..PreferencesPatch::default()
        };
        assert_eq!(
            set_mine(&context(&database, &sam, &clock), originals(Originals::Off)).await,
            PreferencesOutcome::NotAllowed,
            "a person can't switch it"
        );
        let view = unwrap_view(preferences_view(&context(&database, &sam, &clock)).await);
        assert_eq!(view.effective.originals, Originals::Allowed, "the default");

        stuart.mode = ActiveMode::Admin;
        unwrap_view(
            set_household(
                &context(&database, &stuart, &clock),
                originals(Originals::Off),
            )
            .await,
        );
        let sams = unwrap_view(preferences_view(&context(&database, &sam, &clock)).await);
        assert_eq!(sams.effective.originals, Originals::Off);
    }
}
