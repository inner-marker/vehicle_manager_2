# Electronic Signatures & Record-Keeping — Design Plan

Status: draft · Owner: (you) · Applies to: core, client (Dioxus), server (NAS)
Related: `vehicle-maintenance-plan.md` (events, tasks, roles, sync)

> Goal: signatures and records that meet the intent of FAA AC 120-78B (Dec 2024) and the
> content rules of 14 CFR 43.9 / 43.11 / 91.417 "as much as practical" for a self-hosted,
> offline-first app. Cars have no equivalent regulation; the same system gives them
> tamper-evident history for resale/warranty. Not legal advice.
>
> **Constraint: zero paid dependencies.** No paid CAs, TSAs, signing services, or
> per-signer certificates. Everything (trust root, issuing, timestamps, verification)
> is self-hosted and open source.

---

## 1. Standards in scope

| Source | What we take from it |
|---|---|
| **AC 120-78B** §2.1.2, §2.2 | 8 key elements of an e-signature + process guidelines (intent, review, notify, nonrepudiation, sole control, permanence, corrections, archiving) |
| **AC 120-78B** §3.1–3.2 | Record content (what/when/where/who/aircraft M/M/signature), controlled access, preservation, anti-corruption, backup, software revision control, data transfer, record transfer with aircraft |
| **14 CFR 43.9(a)** | Maintenance entry: description (or ref. to data), completion date, name of performer if different, signature + cert number + kind of cert of approver. Signature = RTS for that work only |
| **14 CFR 43.11** | Inspection entries: type/extent, date, total time, signature + cert, required statements; discrepancy list |
| **14 CFR 91.417 / 43.10 / 91.419** | Owner retention (1 yr / until superseded vs. permanent items like TT, AD status), transfer with aircraft |
| **AC 43-9C** | Records may be any format that gives continuity, required content, can take new entries, *provides for signature*, intelligible |
| **E-SIGN Act (15 USC 7001) / UETA (MD adopted)** | Legal definition of e-signature; retained record must stay accurate and reproducible for anyone entitled to it |
| **FIPS 186-5** (referenced by the AC) | Approved signature algorithms — includes **Ed25519** |
| *Reference model:* **21 CFR Part 11** §11.50/11.70/11.200 | Good engineering pattern: signature manifestation (name, time, meaning), signature-record linking, two-factor signing with session rules |

**Applicability note:** AC 120-78B §1.9.5 — Part 43 individuals and Part 91 (non-K) operators need **no OpSpec A025**, but the FAA recommends following the AC, and may question records missing its key elements. So: implement the elements, document the process, no FAA paperwork.

---

## 2. Core principle

**The drawn/typed image is not the signature. It's the appearance.**

The legally meaningful signature is: an authenticated person, deliberately acting, with stated intent, cryptographically bound to the exact content they reviewed. The ink or typed name is a *manifestation* bound into that signature.

```
Signature = Auth(user, device) + Intent statement + Review of exact content
          + Ed25519 sig over canonical(record + meaning + appearance hash + time)
```

---

## 3. Compliance matrix (AC 120-78B → feature)

| AC ref | Requirement | Implementation | Phase |
|---|---|---|---|
| 2.1.2.1 / 2.2.1 | Unique to signer | Per-user Ed25519 key per device, cert issued by server intermediate under offline fleet root | S1 |
| 2.1.1.1 | CA vouches for signer identity | Self-hosted root → intermediate → key cert; Admin approves enrollment | S1 |
| 2.1.2.2 | Identify & authenticate signer | Login + **re-auth at each signing** (PIN or OS biometric unlock of key) | S2 |
| 2.1.2.3 / 2.2.6.1 | Sole control | Private key in OS keystore, non-exportable where possible; never synced; server never holds it | S1 |
| 2.1.2.4 / 2.2.2 | Intent to sign | Signature block shows meaning statement ("Approved for return to service by") + explicit checkbox | S2 |
| 2.1.2.5 | Deliberate action | Draw/adopt + checkbox + auth prompt; no auto-sign, no "remember for session" on RTS | S2 |
| 2.2.3 | Notify signer | Post-sign confirmation screen with signature summary + fingerprint | S2 |
| 2.2.4 | Scope, review before signing | Sign screen renders the full canonical record; Sign enabled only after full scroll; what's rendered is generated from the same struct that's hashed | S2 |
| 2.1.2.6 / 2.2.5 | Nonrepudiation | Asymmetric sig + server receipt + per-vehicle hash chain | S1/S3 |
| 2.1.2.7–8 / 2.2.7 | Permanent; signed doc can't be edited | DB triggers block UPDATE/DELETE on signed rows; content hash in sig | S1 |
| 2.2.6.3 | No additions without new sig | Record hash covers all tasks/parts/attachments; adding a task = new entry | S1 |
| 2.2.6.4 | Revoke departed users | Signed revocation records; server rejects sigs after effective time | S4 |
| 2.2.8 | Corrections | Superseding entry referencing original; original marked *superseded*, kept & verifiable | S4 |
| 2.2.9 / 3.2.7 | Archive / backup | Devices = replicas; signed export bundle; printable PDF; offsite backup (post-MVP) | S5 |
| 2.2.11 / 3.2.8 | Policies & procedures | `docs/records-procedures.md` template (Section 10) | S6 |
| 3.1.1 | Record key elements | Required fields enforced per event type (Section 6) | S1 |
| 3.2.1 | Controlled access | Existing roles; Admin-only key/user mgmt; Tailscale + TLS | existing |
| 3.2.4 | Prevent corruption | SQLite `integrity_check`, hash-chain verify on sync and on startup | S3 |
| 3.2.6 | Software revision control | App/schema version stamped in every signed payload; git-cliff changelog | S1 |
| 3.2.8.9 / 3.2.8.12 | Transfer to new system / with aircraft | Self-verifying export bundle + standalone verifier CLI | S5 |
| 2.2.11.5 / 3.2.8.5 | QC & audit | Scheduled full-chain verification, audit log, annual self-audit checklist | S6 |

---

## 4. Signing UX

### 4.1 Appearance options (user sets up once, can change; each change is a signed "adoption" record)

1. **Drawn** (finger/stylus/mouse)
   - Capture vector strokes via pointer events: `(x, y, t_ms, pressure?)`.
   - Store strokes (canonical JSON) + rendered SVG. Hash strokes into the payload.
   - Options: draw fresh each time, or adopt a saved drawing. Default: saved drawing for routine entries, fresh drawing optional.
   - Reject empty / trivially short strokes; "Clear" and "Undo stroke".
2. **Typed & adopted**
   - Fields: full legal name (prefilled from profile), font, color.
   - Bundle 4–6 **OFL-licensed** script fonts (e.g., Dancing Script, Great Vibes, Caveat, Homemade Apple — verify licenses) — embed in app, never fetched at runtime.
   - Render to SVG; store `{name, font_id, font_version, color}`; hash that.
   - Adoption statement: *"I adopt this as my electronic signature and agree it is the legal equivalent of my handwritten signature."*
3. **Initials** variant for task-level line sign-offs (same mechanics).

Colors: restrict to dark, legible inks (black, blue, dark blue) — printed records must be intelligible.

### 4.2 Signing flow

```
[Record complete] → [Review screen: full rendered entry]
   → scroll to end (enables Sign)
   → choose meaning (constrained by role & record type)
   → signature block: meaning label, name, cert kind/number, date
   → apply appearance (draw or saved)
   → ☐ "I have reviewed this entry and intend to sign it."
   → OS auth prompt (biometric / PIN) → key unlock → sign
   → Confirmation: "Signed. Entry #1234 · fingerprint 3F9A…" + Print/Export
```

Rules:
- Every signature requires re-auth. Batch signing (e.g., 12 task initials in one service event) may use one auth if within 5 min **and** the review screen lists every item being signed (Part 11 §11.200 pattern).
- A signature's meaning is fixed text; free-text "remarks" go in the record, not the signature.

### 4.3 Signature meanings

| Meaning | Who | Notes |
|---|---|---|
| `performed_by` | Maintainer | 43.9(a)(3) "name of person performing" when different from approver |
| `inspected_by` | Maintainer w/ cert | 43.11; RII-style second sign-off |
| `approved_rts` | Maintainer w/ cert | 43.9(a)(4); requires cert kind+number on profile |
| `owner_ack` | Admin/owner | Owner acknowledging discrepancy list / record review |
| `logged_by` | Driver | Non-regulatory (fuel, mileage); may skip re-auth by vehicle setting |
| `supersedes` | Same as original meaning | Correction entries |
| `revoked_by` | Admin | Key/user revocation |

---

## 5. Cryptography & keys

- **Algorithm:** Ed25519 (`ed25519-dalek`), FIPS 186-5 listed. **Hash:** SHA-256 (`sha2`).
- **Canonical encoding:** RFC 8785 JSON Canonicalization (`serde_jcs`). Never sign `serde_json` output directly — key order/whitespace isn't stable.
- **Key storage:** `keyring` crate (macOS/iOS Keychain, Windows Credential Manager, Linux Secret Service); Android Keystore via platform bridge. Encrypt-at-rest fallback (Argon2id-derived key from PIN) for platforms without a keystore — document as weaker.
- **Self-hosted 2-tier PKI (signed JSON certs, not X.509 for MVP; no paid CA ever):**

  ```
  Fleet Root            offline: USB stick + printed recovery code; used only for intermediates
    └─ Intermediate     one per server (NAS) or host-mode desktop; online
         └─ Key cert    one per user+device; private key in OS keystore
              └─ signature on a record
  ```

  | Tier | Lives | Signs | If compromised |
  |---|---|---|---|
  | Fleet Root | Offline (USB + paper) | Intermediate certs, intermediate revocations | Re-root; re-issue everything (rare by design) |
  | Intermediate | NAS server / host-mode desktop | Key certs, key revocations, sync receipts, chain-join entries, export manifests | Root revokes it at `effective_at`; new intermediate issued; earlier signatures stay valid |
  | Key cert | Device keystore | Records, appearance adoptions | Intermediate revokes that cert |

  - **Setup ceremony (first run):** generate root on the setup device → issue first intermediate → write root private key to USB + print recovery code (BIP-39-style words) → wipe root key from disk. App warns until the Admin confirms the offline copy exists.
  - **Enrollment:** each user+device generates its keypair; Admin approves → intermediate issues a **Key Certificate** (signed JSON: `cert_id, issuer_id, user_id, device_id, pubkey, cert_kind, cert_number, roles, valid_from, valid_to`).
  - **Intermediate rotation / host-mode → NAS migration:** plug in root → issue new intermediate → old one gets `valid_to` (not revoked unless compromised). Admin-only, guided flow.
  - **Trust anchor = root fingerprint.** Printed on the PDF logbook cover and in the export bundle; a buyer/IA confirms it out-of-band (e.g., the seller reads it to them). No third party needed.
  - Devices cache the root pubkey, all intermediates, key certs and revocations → full offline verification.
  - Chain validation (in `vm-records`):

  ```rust
  /// Verify a key cert chains to the pinned fleet root.
  /// `chain` is ordered [intermediate, ..., leaf]; the root is never in the chain,
  /// it is pinned by the verifier (device cache or export bundle + fingerprint check).
  pub fn verify_chain<'c>(
      root: &VerifyingKey,
      chain: &'c [KeyCert],
      at: OffsetDateTime,             // signature's claimed time
      revoked: &RevocationSet,        // all revocations known to this verifier
  ) -> Result<&'c KeyCert, ChainError> {
      let mut issuer_key = root;
      for cert in chain {
          // Issuer's Ed25519 signature over the cert's canonical (JCS) bytes.
          cert.verify_issuer_sig(issuer_key)?;
          // Cert must be inside its validity window at signing time.
          cert.check_validity(at)?;
          // Revocation only invalidates signatures made at/after effective_at.
          revoked.check(cert.id, at)?;
          // Next cert must be signed by this one.
          issuer_key = &cert.pubkey;
      }
      // Only leaf (key) certs may sign records; intermediates may not.
      chain.last().filter(|c| c.is_leaf()).ok_or(ChainError::NotLeaf)
  }
  ```

  - A CA vouches for key ↔ person, not for record content; content integrity comes from the signature + hash chain.
- **Time:** device clock is untrusted offline. Store `claimed_at` (in signed payload) and `received_at` (intermediate-signed server receipt). Flag `claimed_at` > `received_at` or skew > 24 h for review. Optional post-MVP: free public RFC 3161 TSAs (e.g., FreeTSA) as an *extra* anchor only — never a dependency.

```rust
use serde::Serialize;
use uuid::Uuid;

/// Exact bytes the signer attests to. Serialized with RFC 8785 (JCS),
/// then SHA-256 hashed, then Ed25519 signed.
/// Any field change => different hash => signature fails verification.
#[derive(Serialize)]
pub struct SignaturePayload<'a> {
    /// Payload schema version; bump on any structural change (AC 3.2.6 revision control).
    pub v: u16,
    /// App version that produced the signature (traceability of software revisions).
    pub app_version: &'a str,
    /// Record (event) being signed.
    pub record_id: Uuid,
    /// SHA-256 (hex) of the canonical record: event + all tasks + parts + attachment hashes.
    pub record_hash: &'a str,
    /// Hash of the previous signed entry in this vehicle's chain (append-only evidence).
    pub prev_entry_hash: Option<&'a str>,
    /// Meaning of the signature, e.g. "approved_rts".
    pub meaning: &'a str,
    /// Exact intent statement displayed to the signer.
    pub statement: &'a str,
    /// Signer identity snapshot at signing time (cert may change later).
    pub signer_id: Uuid,
    pub signer_name: &'a str,
    pub cert_kind: Option<&'a str>,   // e.g. "Repairman (Light-Sport), Maintenance"
    pub cert_number: Option<&'a str>,
    /// Hash of the appearance (ink strokes or typed adoption record).
    pub appearance_hash: &'a str,
    /// Key certificate id used; links to user + device.
    pub key_cert_id: Uuid,
    /// Device-reported UTC time (RFC 3339). Server adds received_at separately.
    pub claimed_at: &'a str,
    /// Set only for corrections: the entry this one supersedes.
    pub supersedes: Option<Uuid>,
}
```

```rust
use ed25519_dalek::{Signer, SigningKey, Signature};
use sha2::{Digest, Sha256};

/// Canonicalize, hash, and sign a payload.
/// Returns (payload_hash_hex, signature). Errors are surfaced, never unwrapped.
pub fn sign_payload(
    key: &SigningKey,
    payload: &SignaturePayload<'_>,
) -> Result<(String, Signature), SignError> {
    // RFC 8785 gives deterministic bytes across platforms/serde versions.
    let canonical = serde_jcs::to_vec(payload).map_err(SignError::Canonicalize)?;
    // Sign the digest rather than raw bytes so the hash can be shown/compared in UI and exports.
    let digest = Sha256::digest(&canonical);
    let sig = key.sign(&digest);
    Ok((hex::encode(digest), sig))
}
```

Put all of this in a dedicated crate: **`vm-records`** (no UI, no DB) → testable, reused by client, server, and the verifier CLI.

---

## 6. Data model additions

```
appearance            id, user_id, kind(drawn|typed|initials), data_json, svg, hash,
                      adopted_at, adoption_sig, retired_at
fleet_root            id, pubkey, fingerprint, created_at        (public half only; private key never stored)
intermediate_cert     id, root_id, pubkey, host_label, valid_from, valid_to, root_sig
key_cert              id, issuer_id (intermediate), user_id, device_id, pubkey, cert_kind,
                      cert_number, roles_json, valid_from, valid_to, issuer_sig
signature             id, record_id, payload_json (canonical), payload_hash,
                      sig, key_cert_id, meaning, claimed_at
server_receipt        signature_id, received_at, intermediate_id, receipt_sig
revocation            id, target_cert_id, target_kind (intermediate|key), effective_at,
                      reason, issuer_id, issuer_sig        (root signs intermediate revocations)
record_state (view)   record_id → draft | signed | superseded(by) | voided
audit_log             id, at, user_id, device_id, action, target, detail_json  (append-only)
```

Record states: `draft` (editable) → `signed` (frozen) → optionally `superseded`. Drafts never sync as signed; only signed entries enter the vehicle chain.

**Required-field enforcement (AC 3.1.1 + 43.9/43.11)** per event type, validated before Sign is enabled:
- what (description or reference to data), when (completion date), where (location, optional for cars), who (performer, approver), vehicle/aircraft identity (make/model/serial/reg), meter reading (odometer or TT/Hobbs/tach), signature + cert kind/number where required.
- Aircraft templates: 43.9 maintenance, 43.11 inspection (incl. required statements, discrepancy list), E-AB/LSA condition inspection per operating limitations, AD compliance entry.

**Immutability in SQLite:**

```sql
-- Block edits/deletes of anything already signed. Corrections are new rows.
CREATE TRIGGER signature_no_update BEFORE UPDATE ON signature
BEGIN SELECT RAISE(ABORT, 'signature rows are immutable'); END;

CREATE TRIGGER signature_no_delete BEFORE DELETE ON signature
BEGIN SELECT RAISE(ABORT, 'signature rows are immutable'); END;

-- Block edits to an event once a signature references it.
CREATE TRIGGER event_frozen_after_sign BEFORE UPDATE ON event
WHEN EXISTS (SELECT 1 FROM signature WHERE record_id = OLD.id)
BEGIN SELECT RAISE(ABORT, 'event is signed; create a superseding entry'); END;
-- Repeat for task, part_used, attachment tables keyed to event.
```

Triggers stop accidents; the hash chain catches anyone who bypasses them.

---

## 7. Sync & verification (fits existing offline-first design)

- Signed entries are immutable and content-addressed → **no merge conflicts** on signed data.
- Per-vehicle chain: each signed entry includes `prev_entry_hash`. Two devices signing offline for the same vehicle create a fork → server accepts both, orders by `received_at`, and records a **chain-join** entry (server-signed) referencing both heads. UI flags the fork; nothing is lost.
- Server on receipt: verify chain (key cert → intermediate → pinned root) → not revoked at `claimed_at` → signature valid → record hash matches → issue intermediate-signed receipt. Reject and log otherwise.
- Clients re-verify everything pulled from the server (don't trust the NAS — matches existing threat model).
- Startup + scheduled: full chain verify + `PRAGMA integrity_check`; mismatch → read-only mode + alert, never push local data over a corrupted master (ties into existing corruption-detection goal).

---

## 8. Corrections & revocation

**Correction (AC 2.2.8):**
1. User picks a signed entry → "Correct this entry".
2. App clones it into a new draft with `supersedes = original_id` and a required reason.
3. New entry is signed normally. Original's state becomes `superseded`; it stays visible (struck-through style) and cryptographically verifiable, but no longer counts for due-date or compliance calculations.
4. "Void" = correction with no replacement content (signed reason only).

**Revocation (AC 2.2.6.4):** Admin issues a revocation with `effective_at` — signed by the intermediate for key certs, by the offline root for intermediates. Signatures with `claimed_at` before it remain valid; signatures received afterward but claiming an earlier time get flagged for Admin review (possible backdating with a lost device or stolen intermediate).

---

## 9. Export, print, transfer

- **Signed export bundle** (`.vmrec`, a zip): `records.jsonl` (canonical), `signatures.jsonl`, `appearances/`, `attachments/`, `certs.json` (root pubkey + fingerprint, intermediates, key certs, revocations), `manifest.json` (hashes of all files, signed by the current intermediate). Self-verifying, readable without the app. Satisfies 43.10/91.419 transfer and AC 3.2.8.9 system migration.
- **Verifier CLI** (`vm-verify`, same `vm-records` crate, free/open source): `vm-verify bundle.vmrec --root <fingerprint>` → pass/fail per entry. Ships with the export so a buyer/IA can check records.
- **PDF logbook view:** cover page shows fleet root fingerprint; per vehicle/component, chronological, each entry showing content, appearance image, name, cert kind/number, meaning, date, short fingerprint, and a QR/footer pointing to the bundle hash. This is the "intelligible" paper backup (AC 3.2.7 lists printed records as a valid backup).
- **PAdES (post-MVP, optional):** PDF signatures chained to the self-hosted root. Acrobat will show "validity unknown" unless the viewer imports the root — acceptable, since `vm-verify` is the authoritative check. Paid AATL certs (Acrobat green check) are explicitly out of scope.

---

## 10. Procedures document (create `docs/records-procedures.md`)

AC 2.2.11 / 3.2.8 expects written procedures. Even though no A025 is needed, keep a short owner-maintained doc covering:
1. Records kept in the system and which are signed (and by what meaning).
2. System description: devices, NAS server, network (Tailscale), software versions.
3. Responsible person(s) and authorized users/roles.
4. How signatures are applied, corrected, revoked.
5. Backup method and how to produce records for FAA/NTSB/buyer (export bundle + PDF).
6. QC/audit: annual self-audit — run full verification, spot-check N entries against required content, confirm backups restore, review audit log.
7. Software change control: how app updates are tested before use on live records.
8. Transition from paper: how legacy logbooks are referenced (scanned + "continued in electronic system as of <date>" signed entry in both).

Ship a template with the app; Admin fills it in.

---

## 11. Phasing

| Phase | Deliverable | Done when |
|---|---|---|
| **S1** | `vm-records` crate: payload types, JCS, hashing, sign/verify, root/intermediate/key certs, chain validation, record hash chain; DB tables + triggers; root setup ceremony + offline export | Unit + property tests; golden test vectors pass on all target OSes; root private key absent from disk after setup |
| **S2** | Appearance capture (draw, typed+font+color, initials), signing flow, re-auth, confirmation | Manual test script covers every row of §3 marked S2 |
| **S3** | Server verification, receipts, client re-verification, fork handling, startup integrity check | Tampered DB row is detected on both client and server |
| **S4** | Corrections, voids, key + intermediate revocation, intermediate rotation / host→NAS migration, Admin review queue | Superseded entries excluded from due calculations; rotated intermediate keeps old signatures valid |
| **S5** | Export bundle, `vm-verify` CLI, PDF logbook | Bundle verifies on a clean machine without the app |
| **S6** | Procedures template, audit log UI, scheduled self-audit | Annual audit checklist runnable from Admin mode |
| Post-MVP | Free RFC 3161 TSA as optional extra anchor, PAdES under self-hosted root, X.509 encoding of certs, hardware keys (YubiKey/passkeys), cross-certification (mechanic trusted across fleets) | All remain free/self-hosted |

---

## 12. Tests to require (add to AGENTS.md)

- Golden vectors: fixed payload → fixed JCS bytes → fixed hash → fixed signature (Ed25519 is deterministic).
- Property tests (`proptest`): any single-field mutation of a signed record fails verification.
- Trigger tests: UPDATE/DELETE on signed rows aborts.
- Revocation boundary tests (before/at/after `effective_at`, clock skew), for both key certs and intermediates.
- Chain tests: wrong issuer, expired intermediate, intermediate used as leaf, unknown root → all rejected.
- Fork test: two offline devices sign same vehicle → both preserved, join entry created.
- Export round-trip: export → verify → import → identical hashes.
- Renderer parity: review-screen content and signed content come from the same struct (snapshot test).

---

## 13. Open decisions

- [ ] Drawn signature: fresh each signing vs. saved adopted drawing (default proposed: saved, fresh optional).
- [ ] Batch-signing window (proposed 5 min, single review screen).
- [ ] Driver `logged_by` entries: require re-auth or not (per-vehicle setting?).
- [ ] Android keystore bridge approach under Dioxus.
- [ ] Font shortlist + license check.
- [ ] Whether `approved_rts` is hard-blocked without cert kind/number on the profile (proposed: yes for aircraft, n/a for cars).
- [ ] Root recovery format: word list vs. QR vs. both; whether to support splitting it (Shamir) across 2 people.
- [ ] Intermediate validity period (proposed: 5 years, rotate early on host change).
- [ ] Vehicle "regulatory profile": `none` (car) vs. `faa_part_43_91` (aircraft) to toggle required fields and templates.
