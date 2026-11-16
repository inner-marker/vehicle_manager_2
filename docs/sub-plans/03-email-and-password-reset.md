# Email and Password Reset Sub-Plan

Sub-plan for the vehicle maintenance manager (Dioxus/Rust, self-hosted on NAS via Docker). Lives alongside `vehicle-maintenance-plan.md`.

## Goals

- Email is **optional**. The app must be fully usable with no email configured.
- No paid services required. Any standard SMTP provider works (Gmail with an app password is the reference setup).
- Password reset always has a non-email path.

## Password Reset Paths

1. **Log-based temporary code (existing).** A temporary code is written to a log file within the NAS folder structure.
   - Short expiry, single use.
   - Tight file permissions on the log.
   - Codes must never appear in logs exported for debugging (redact, or write to a separate file).
2. **Admin-issued reset.** An Admin triggers a reset for a user and receives a one-time code to hand over out of band.
3. **Email reset (when SMTP is configured).** See below.

## New Users by Email (the human, 2026-11-08; `docs/decisions.md` #072)

- When email is configured, an Admin adding a user can send a **"Create your new password" link** instead of handing over a temporary password (#046).
- The link works **once** and **expires after 10 minutes**; the server stores only its hash, as for the reset token below. An expired link is resent by the Admin.
- Following it sets the password and signs the person in on that browser. Their first desktop sign-in then pairs that desktop as usual (server address, username, password; #072).
- Without email, the temporary password stays the way.

## Email Reset Flow

1. User requests a reset by username or email.
2. Response is always the same generic message ("If that account exists, a reset email was sent") to prevent account enumeration.
3. Server generates a cryptographically random token and stores **only its hash**, with the user ID and an expiry (~30 minutes).
4. Email contains a link with the raw token, pointing at the configured base URL (e.g. the NAS web UI over Tailscale).
5. On submit, the server verifies the hash and expiry, sets the new password, deletes the token, and invalidates the user's other sessions.
6. Rate-limit reset requests per account and per source.
7. Every reset (any path) is recorded in the in-app debug log, never including the token.

## SMTP Configuration

- Settings: host, port, security mode (STARTTLS on 587 or implicit TLS on 465), username, password/app password, from address, app base URL.
- Supplied via Docker environment variables or Docker secrets; secrets never stored in plain text in the main database.
- Suggested crate: `lettre`.
- Admin UI shows whether email is configured and offers a "send test email" button.
- Gmail notes: requires 2-step verification and an app password; personal accounts have a daily sending limit (roughly 500/day).
- Free-tier API alternatives if preferred: Brevo, Resend.

## Phasing

### Early phases

- Basic email password reset as described above.
- Simple per-user email on/off toggle.
- SMTP config plus test-email button.

### Later phases (approaching MVP)

- Robust per-user notification preferences (event types, vehicles, due-soon thresholds, digest vs. immediate).
- Email notifications for maintenance due, tied into the in-app notification system.

### Post-MVP

- Mobile push notifications (planned separately).

## Open Questions

- Base URL handling when accessed via Tailscale vs. local network.
- Whether host-mode desktop installs get the same SMTP options.
