//! Shops and the household's labor rate on the server (phase-6.md 2f; decision #089): who may
//! see and change them, the checks, changing and removing a shop, and the rate being the
//! household's alone.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod shops {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{PreferencesOutcome, ShopsOutcome, ShopsView};
    use domain::label::Label;
    use domain::preferences::{Choice, PreferencesPatch};
    use domain::shop::{LaborRate, Shop, ShopProblem};
    use domain::test_support::{ManualClock, test_id};
    use server::assets::AssetsContext;
    use server::health::Health;
    use server::preferences::{PreferencesContext, set_mine};
    use server::sessions::SignedIn;
    use server::shops;
    use storage::Database;

    fn context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> AssetsContext<'a> {
        AssetsContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            clock,
            origin: server::origin::Origin::Online,
        }
    }

    fn in_mode(user: &SignedIn, mode: ActiveMode) -> SignedIn {
        SignedIn {
            mode,
            ..user.clone()
        }
    }

    fn rate(minor: i64) -> LaborRate {
        LaborRate::new(minor).unwrap()
    }

    fn shop(name: &str, contact: Option<&str>, labor_rate: Option<LaborRate>) -> Shop {
        Shop {
            name: Label::parse(name).unwrap(),
            contact: contact.map(str::to_owned),
            labor_rate,
        }
    }

    async fn view(context: &AssetsContext<'_>) -> ShopsView {
        let ShopsOutcome::Shops { view } = shops::list(context).await else {
            panic!("no list");
        };
        view
    }

    #[tokio::test]
    async fn maintainers_keep_the_shops_and_everyone_sees_them() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let keeper = context(&database, &maintainer, &clock);
        let sams = context(&database, &sam, &clock);

        // Nothing yet; Sam (a Driver) may look but not change.
        let empty = view(&sams).await;
        assert!(empty.shops.is_empty() && empty.household_rate.is_none());
        assert!(!empty.may_manage);
        assert!(view(&keeper).await.may_manage);
        assert_eq!(
            shops::add(&sams, shop("Joe's Auto", None, None)).await,
            ShopsOutcome::NotAllowed
        );
        assert_eq!(
            shops::set_household_rate(&sams, Some(rate(9_500))).await,
            ShopsOutcome::NotAllowed
        );
        // Jordan in Driver mode can't either: the mode counts.
        let jordan_driving = in_mode(&jordan, ActiveMode::Driver);
        assert_eq!(
            shops::add(
                &context(&database, &jordan_driving, &clock),
                shop("Joe's Auto", None, None)
            )
            .await,
            ShopsOutcome::NotAllowed
        );

        // Added, by name; a second "joe's auto" is refused.
        let added = shops::add(
            &keeper,
            shop("Joe's Auto", Some("541-555-0100"), Some(rate(12_000))),
        )
        .await;
        assert!(matches!(added, ShopsOutcome::Saved { .. }));
        let added = shops::add(&keeper, shop("Tire Barn", None, None)).await;
        assert!(matches!(added, ShopsOutcome::Saved { .. }));
        assert_eq!(
            shops::add(&keeper, shop("joe's auto", None, None)).await,
            ShopsOutcome::Invalid {
                problem: ShopProblem::NameTaken
            }
        );
        assert_eq!(
            shops::add(&keeper, shop("Dealer", Some(" "), None)).await,
            ShopsOutcome::Invalid {
                problem: ShopProblem::BadContact
            }
        );
        shops::set_household_rate(&keeper, Some(rate(9_500))).await;
        let seen = view(&sams).await;
        let names: Vec<&str> = seen
            .shops
            .iter()
            .map(|(_, shop)| shop.name.as_str())
            .collect();
        assert_eq!(names, vec!["Joe's Auto", "Tire Barn"]);
        assert_eq!(seen.household_rate, Some(rate(9_500)));
        assert_eq!(seen.shops[0].1.labor_rate, Some(rate(12_000)));
    }

    #[tokio::test]
    async fn a_shop_changes_and_goes_and_the_rate_clears() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let keeper = context(&database, &maintainer, &clock);
        let sams = context(&database, &sam, &clock);
        let ShopsOutcome::Saved { id: joes } = shops::add(
            &keeper,
            shop("Joe's Auto", Some("541-555-0100"), Some(rate(12_000))),
        )
        .await
        else {
            panic!("not added");
        };
        let ShopsOutcome::Saved { id: barn } =
            shops::add(&keeper, shop("Tire Barn", None, None)).await
        else {
            panic!("not added");
        };
        shops::set_household_rate(&keeper, Some(rate(9_500))).await;

        // Joe's changes its rate and drops its contact; renaming Tire Barn to Joe's is refused.
        assert_eq!(
            shops::edit(&keeper, joes, shop("Joe's Auto", None, Some(rate(13_000)))).await,
            ShopsOutcome::Saved { id: joes }
        );
        assert_eq!(
            shops::edit(&keeper, barn, shop("JOE'S AUTO", None, None)).await,
            ShopsOutcome::Invalid {
                problem: ShopProblem::NameTaken
            }
        );
        assert_eq!(
            shops::edit(&sams, joes, shop("Joe's", None, None)).await,
            ShopsOutcome::NotAllowed
        );
        let joes_now = database.shop(joes).await.unwrap().unwrap();
        assert_eq!(joes_now.contact, None);
        assert_eq!(joes_now.labor_rate, Some(rate(13_000)));

        // Removed: gone from the list; again, or a shop that never was, isn't found.
        assert_eq!(shops::delete(&keeper, barn).await, ShopsOutcome::Done);
        assert_eq!(view(&keeper).await.shops.len(), 1);
        assert_eq!(shops::delete(&keeper, barn).await, ShopsOutcome::NotFound);
        assert_eq!(
            shops::edit(&keeper, test_id(77), shop("Ghost", None, None)).await,
            ShopsOutcome::NotFound
        );

        // The household's rate cleared: none.
        assert_eq!(
            shops::set_household_rate(&keeper, None).await,
            ShopsOutcome::Done
        );
        assert_eq!(view(&sams).await.household_rate, None);
    }

    #[tokio::test]
    async fn the_labor_rate_is_the_households_alone() {
        let (_temp, database, _jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let preferences = PreferencesContext {
            database: &database,
            database_id: test_id(900),
            user: &sam,
            health: Health::Healthy,
            clock: &clock,
            origin: server::origin::Origin::Online,
        };
        let patch = PreferencesPatch {
            labor_rate: Some(Choice::Set(rate(1))),
            ..PreferencesPatch::default()
        };
        assert_eq!(
            set_mine(&preferences, patch).await,
            PreferencesOutcome::NotAllowed
        );
    }
}
