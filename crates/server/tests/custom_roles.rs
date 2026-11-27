//! A household's custom roles on the server (phase-6.md 4c; sub-plan 05; decision #091): who
//! may change them, their checks, granting one, a change reaching its holders, and removing it.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod custom_roles {
    use crate::common::{NOW, setup};
    use domain::access::{
        ActiveMode, AssetAction, AssetScope, GrantTarget, Permissions, RoleGrant, available_modes,
        permits_on_asset,
    };
    use domain::api::{RolesOutcome, UserAdminOutcome};
    use domain::custom_role::{CustomRole, CustomRoleId, RoleProblem};
    use domain::label::Label;
    use domain::test_support::{ManualClock, test_id};
    use domain::user::UserId;
    use server::custom_roles::{add, delete, edit, list};
    use server::health::Health;
    use server::sessions::SignedIn;
    use server::user_admin::{AdminContext, set_custom_role};
    use storage::Database;

    fn context<'a>(
        database: &'a Database,
        person: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> AdminContext<'a> {
        AdminContext {
            database,
            admin: person,
            health: Health::Healthy,
            clock,
        }
    }

    fn in_admin(person: &SignedIn) -> SignedIn {
        SignedIn {
            mode: ActiveMode::Admin,
            ..person.clone()
        }
    }

    fn chauffeur() -> CustomRole {
        CustomRole {
            name: Label::parse("Chauffeur").unwrap(),
            permissions: Permissions::of(&[AssetAction::LogFuel, AssetAction::LogReading]),
        }
    }

    async fn added(office: &AdminContext<'_>, role: CustomRole) -> CustomRoleId {
        let outcome = add(office, role).await;
        let RolesOutcome::Saved { id } = outcome else {
            panic!("not added: {outcome:?}");
        };
        id
    }

    async fn grants_of(database: &Database, user: UserId) -> Vec<RoleGrant> {
        database
            .grants_for(user)
            .await
            .unwrap()
            .into_iter()
            .map(|(_, grant)| grant)
            .collect()
    }

    fn anywhere() -> AssetScope {
        AssetScope {
            id: test_id(70),
            group: test_id(71),
        }
    }

    #[tokio::test]
    async fn only_the_admin_in_admin_mode_changes_roles_and_everyone_sees_them() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        for person in [&jordan, &sam] {
            assert_eq!(
                add(&context(&database, person, &clock), chauffeur()).await,
                RolesOutcome::NotAllowed
            );
        }
        let admin = in_admin(&jordan);
        let office = context(&database, &admin, &clock);
        let id = added(&office, chauffeur()).await;
        let RolesOutcome::Roles { view } = list(&context(&database, &sam, &clock)).await else {
            panic!("Sam sees no roles");
        };
        assert_eq!(view.roles.len(), 1);
        assert_eq!(view.roles[0].id, id);
        assert!(!view.may_manage);
        assert_eq!(
            delete(&context(&database, &sam, &clock), id).await,
            RolesOutcome::NotAllowed
        );
    }

    #[tokio::test]
    async fn roles_are_checked() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_admin(&jordan);
        let office = context(&database, &admin, &clock);
        let id = added(&office, chauffeur()).await;
        let again = CustomRole {
            name: Label::parse("chauffeur").unwrap(),
            ..chauffeur()
        };
        assert_eq!(
            add(&office, again).await,
            RolesOutcome::Invalid {
                problem: RoleProblem::NameTaken
            }
        );
        let boss = CustomRole {
            name: Label::parse("Boss").unwrap(),
            permissions: Permissions::of(&[AssetAction::ManageAsset]),
        };
        assert_eq!(
            edit(&office, id, boss).await,
            RolesOutcome::Invalid {
                problem: RoleProblem::AdminOnly
            }
        );
        assert_eq!(
            edit(&office, test_id(99), chauffeur()).await,
            RolesOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn a_granted_role_allows_what_it_says_and_follows_its_changes_until_removed() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_admin(&jordan);
        let office = context(&database, &admin, &clock);
        let id = added(&office, chauffeur()).await;
        assert_eq!(
            set_custom_role(&office, sam.user, GrantTarget::AllAssets, id).await,
            UserAdminOutcome::Done
        );
        assert_eq!(
            set_custom_role(&office, sam.user, GrantTarget::AllAssets, test_id(98)).await,
            UserAdminOutcome::NotFound,
            "no such role"
        );
        let grants = grants_of(&database, sam.user).await;
        assert_eq!(grants.len(), 1, "it replaced Sam's Driver role");
        let may = |grants: &[RoleGrant], action| {
            permits_on_asset(grants, ActiveMode::Driver, action, anywhere())
        };
        assert!(may(&grants, AssetAction::LogFuel));
        assert!(!may(&grants, AssetAction::OpenSquawk));
        assert_eq!(available_modes(&grants), [ActiveMode::Driver]);
        let RolesOutcome::Roles { view } = list(&office).await else {
            panic!("no list");
        };
        assert_eq!(view.roles[0].holders, 1);

        // Squawks added to the role: Sam has them at once.
        let wider = CustomRole {
            permissions: Permissions::of(&[
                AssetAction::LogFuel,
                AssetAction::LogReading,
                AssetAction::OpenSquawk,
            ]),
            ..chauffeur()
        };
        assert_eq!(edit(&office, id, wider).await, RolesOutcome::Saved { id });
        assert!(may(
            &grants_of(&database, sam.user).await,
            AssetAction::OpenSquawk
        ));

        // Removed: its grants go with it.
        assert_eq!(delete(&office, id).await, RolesOutcome::Done);
        assert!(grants_of(&database, sam.user).await.is_empty());
        assert_eq!(delete(&office, id).await, RolesOutcome::NotFound);
    }

    #[tokio::test]
    async fn an_apprentice_doesnt_manage_shops() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_admin(&jordan);
        let office = context(&database, &admin, &clock);
        let apprentice = CustomRole {
            name: Label::parse("Apprentice").unwrap(),
            permissions: Permissions::of(&[AssetAction::LogMaintenance]),
        };
        let id = added(&office, apprentice).await;
        set_custom_role(&office, sam.user, GrantTarget::AllAssets, id).await;
        let sam_now = SignedIn {
            grants: grants_of(&database, sam.user).await,
            mode: ActiveMode::Maintainer,
            ..sam.clone()
        };
        assert!(!server::shops::may_manage(&sam_now));
        let jordan_maintainer = SignedIn {
            mode: ActiveMode::Maintainer,
            ..jordan.clone()
        };
        assert!(server::shops::may_manage(&jordan_maintainer));
    }
}
