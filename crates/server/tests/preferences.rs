//! Display preferences against a real database, through the command pipeline.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod preferences {
    use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::api::{PreferencesOutcome, PreferencesView};
    use domain::command::Change;
    use domain::preferences::{
        Choice, CurrencyCode, DateFormat, DistanceUnit, EconomyUnit, Language, PreferencesPatch,
        PressureUnit, VolumeUnit,
    };
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use server::commands::issue;
    use server::health::Health;
    use server::login_limiter::LoginLimiter;
    use server::passwords::hash_password;
    use server::preferences::{PreferencesContext, set_household, set_mine};
    use server::sessions::{SignedIn, sign_in};
    use storage::users::{RoleGrantWriter, UserWriter};
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    /// A database with Admin `stuart` (1) and Driver `sam` (2), both signed in (Driver mode).
    async fn setup() -> (tempfile::TempDir, Database, SignedIn, SignedIn) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let clock = ManualClock::new(NOW);
        for (n, name, role) in [(1, "stuart", Role::Admin), (2, "sam", Role::Driver)] {
            let user = User {
                username: Username::parse(name).unwrap(),
                display_name: name.into(),
                disabled: false,
                must_change_password: false,
            };
            let grant = RoleGrant {
                user: test_id(n),
                target: GrantTarget::AllAssets,
                role,
            };
            issue(
                &database,
                &UserWriter,
                Change::Create {
                    id: test_id(n),
                    values: UserPatch::all(user),
                },
                test_id(901),
                None,
                Health::Healthy,
                &clock,
            )
            .await
            .unwrap();
            issue(
                &database,
                &RoleGrantWriter,
                Change::Create {
                    id: test_id(n + 100),
                    values: RoleGrantPatch::all(grant),
                },
                test_id(901),
                None,
                Health::Healthy,
                &clock,
            )
            .await
            .unwrap();
            database
                .set_password_hash(
                    test_id(n),
                    &hash_password("a long password".into()).await.unwrap(),
                    NOW,
                )
                .await
                .unwrap();
        }
        let limiter = LoginLimiter::new();
        let (_, stuart) = sign_in(
            &database,
            &limiter,
            "stuart",
            "a long password".into(),
            "a",
            NOW,
        )
        .await
        .unwrap();
        let (_, sam) = sign_in(
            &database,
            &limiter,
            "sam",
            "a long password".into(),
            "b",
            NOW,
        )
        .await
        .unwrap();
        (temp, database, stuart, sam)
    }

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
}
