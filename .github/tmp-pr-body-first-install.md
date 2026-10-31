## Purpose

Let a person configure a fresh Free Local installation from the app: detect that provisioning is required, read the native authorization from the private installation directory, create the organization + administrator + credential + grants in one transaction, and land on the login screen — without the token ever crossing IPC, logs, files written by the app, or the renderer.

## What is included

**Native authorization (`pos-storage-sqlite::native_authorization`)**
Reads `provisioning-authorization.txt` from the installation directory and enforces custody on Unix: no symlink, owner must be the effective user, no group/other bits on directory or file. Classifies the right against SQLite as ready, expired, revoked, consumed or unknown, and rejects a file whose digest does not match the persisted authorization. Windows stays **fail-closed**.

**Desktop (`bootstrap_status`, `bootstrap_install`)**
`bootstrap_status` never mutates state. `bootstrap_install` requires a live right, re-checks that the installation still needs provisioning, derives the capability from the token read natively, and consumes the authorization in the same `BEGIN IMMEDIATE` unit of work that writes organization, `installation_identity`, administrator, credential, role, 59 grants and the `bootstrap.completed` SYSTEM audit. The authorization file is removed after commit. The token never appears in any DTO, request, log or bindings.

**React**
Setup wizard with explicit states: ready, awaiting authorization, expired, revoked, consumed, invalid, insecure permissions, unsupported platform. Nothing silently resets to “new installation”. Credentials are validated in Rust; the client only checks password confirmation and the 12-character minimum.

## Verification

| Check | Result |
| --- | --- |
| `pos-storage-sqlite::native_authorization_tests` (Linux, real SQLite) | 4/4: private file, insecure directory, insecure file, symlink, missing, digest mismatch, expiry, and full file → bootstrap → login → reopen journey |
| `cargo fmt`, `clippy -D warnings`, workspace tests | clean, 0 failures |
| Desktop tests (Windows) | 37/37, including platform gating and “no identity created without authorization” |
| Frontend lint / typecheck / tests / builds | clean, 218 passed, both builds ok |

## Explicit limits

- Windows first-install remains disabled until the DACL/owner component exists; this PR does not weaken the existing fail-closed behaviour.
- No graphical WebView2 E2E in this PR.
- Timezone is fixed to UTC and locale to `es-MX`; currency is selectable (MXN/USD/EUR). Business timezone/locale selection is a later slice.
- The authorization must be placed by the existing `pos-local --provision-token-issue` CLI; no UI path accepts or copies the token.
