//! A PIN for Admin mode (phase-6.md 4e; sub-plan 04 §4.2): choosing one, entering with it,
//! wrong ones counting toward the sign-in limit, and going back to the password.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod admin_pin {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::user::PinError;
    use server::admin_pin::{AdminPinError, has_admin_pin, set_admin_pin};
    use server::login_limiter::{LoginLimiter, MAX_FAILURES};
    use server::sessions::{SignInError, SwitchModeError, sign_in, switch_mode_limited};

    const PASSWORD: &str = "a long password";

    #[tokio::test]
    async fn an_admin_chooses_a_pin_and_enters_admin_mode_with_it() {
        let (_temp, database, jordan, sam) = setup().await;
        let limiter = LoginLimiter::new();
        let enter = |secret: &str| {
            switch_mode_limited(
                &database,
                &limiter,
                &jordan,
                ActiveMode::Admin,
                Some(secret.into()),
                "laptop",
                NOW,
            )
        };
        assert_eq!(has_admin_pin(&database, &jordan).await, Ok(false));
        assert_eq!(
            set_admin_pin(&database, &sam, PASSWORD.into(), Some("123456".into())).await,
            Err(AdminPinError::NotAllowed),
            "a Driver has no Admin mode"
        );
        assert_eq!(
            set_admin_pin(&database, &jordan, "wrong".into(), Some("123456".into())).await,
            Err(AdminPinError::WrongPassword)
        );
        assert_eq!(
            set_admin_pin(&database, &jordan, PASSWORD.into(), Some("12345".into())).await,
            Err(AdminPinError::Invalid(PinError::NotSixToTwelveDigits))
        );
        assert_eq!(
            set_admin_pin(&database, &jordan, PASSWORD.into(), Some("482916".into())).await,
            Ok(())
        );
        assert_eq!(has_admin_pin(&database, &jordan).await, Ok(true));

        // The PIN now, not the password.
        assert_eq!(enter(PASSWORD).await, Err(SwitchModeError::WrongPin));
        assert_eq!(enter("482916").await, Ok(ActiveMode::Admin));

        // Back to the password: the PIN stops working.
        assert_eq!(
            set_admin_pin(&database, &jordan, PASSWORD.into(), None).await,
            Ok(())
        );
        assert_eq!(has_admin_pin(&database, &jordan).await, Ok(false));
        assert_eq!(enter("482916").await, Err(SwitchModeError::WrongPassword));
        assert_eq!(enter(PASSWORD).await, Ok(ActiveMode::Admin));
    }

    #[tokio::test]
    async fn wrong_pins_lock_admin_mode_and_signing_in() {
        let (_temp, database, jordan, _sam) = setup().await;
        let limiter = LoginLimiter::new();
        set_admin_pin(&database, &jordan, PASSWORD.into(), Some("482916".into()))
            .await
            .unwrap();
        for _ in 0..MAX_FAILURES {
            assert_eq!(
                switch_mode_limited(
                    &database,
                    &limiter,
                    &jordan,
                    ActiveMode::Admin,
                    Some("000000".into()),
                    "laptop",
                    NOW,
                )
                .await,
                Err(SwitchModeError::WrongPin)
            );
        }
        let locked = switch_mode_limited(
            &database,
            &limiter,
            &jordan,
            ActiveMode::Admin,
            Some("482916".into()),
            "laptop",
            NOW,
        )
        .await;
        assert!(
            matches!(locked, Err(SwitchModeError::RateLimited { .. })),
            "even the right PIN waits: {locked:?}"
        );
        let signing_in =
            sign_in(&database, &limiter, "jordan", PASSWORD.into(), "phone", NOW).await;
        assert!(
            matches!(signing_in, Err(SignInError::RateLimited { .. })),
            "the person's sign-in waits too"
        );
        // Leaving for Driver mode needs nothing, and isn't limited.
        assert_eq!(
            switch_mode_limited(
                &database,
                &limiter,
                &jordan,
                ActiveMode::Driver,
                None,
                "laptop",
                NOW,
            )
            .await,
            Ok(ActiveMode::Driver)
        );
    }
}
