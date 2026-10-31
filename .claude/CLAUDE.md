# Project knowledge for Claude

Rolx time tracking app for small systems (Rust, Slint UI), desktop and a RISC-V board.
User-facing build and configuration docs are in `README.md`; this file holds what is
not obvious from the code.

## Workspace

- `tracker`: lib + desktop bin, the application with the Slint UI.
- `device_flow`: lib (from teammates), Google device/native flow and Rolx sign-in. `device_flow_cli` is its test binary.
- `api`: lib, progenitor-generated Rolx client from the OpenAPI spec (`Client::with_token`). Used by `tracker` (module `rolx`) and `device_flow_cli`.

### tracker

- Features: `desktop` (default; winit + femtovg) and `linuxkms` (`backend-linuxkms-noseat` + software renderer, for the RISC-V board). The board build imports `tracker` with `default-features = false, features = ["linuxkms"]`. The `linuxkms` backend already draws a mouse pointer via libinput (it shows after the first mouse move). That variant cannot be built on the dev machine (no libudev-dev).
- Modules:
  - `config`: `config.toml` from `/etc/rolx/tracker` first, then `$HOME/.config/rolx/tracker`. Argon2 admin password hash (`tracker --hash-password`), server and device client settings, WLAN and log settings.
  - `connectivity`: TCP check of the server every 10 s, drives the online indicator.
  - `login`: wraps `device_flow` (in-memory `SessionStore`, tokio runtime per login thread, cancel by dropping the pending login, e-mail must match the typed user name).
  - `rolx`: activities (`GET /api/v1/Activity/suitable/{userId}/{date}` plus `GET /api/v1/activity/favourite`, merged by `merge()`: dedup by id, favourites only when open on that day, sorted by name, favourites marked with `★` in the UI label) and work records (`GET /api/v1/WorkRecord/...`) through the `api` client; blocking, so call it from a thread. Rolx counts **seconds**: `RecordEntry.duration` is the length, `begin` the seconds since midnight (`Pause` must be null when `begin` is set, entries with `duration` 0 are dropped). The record range is **half-open** (`begin <= date < end`): asking for `{date}..{date}` yields nothing, and since the PUT replaces the day's entries that would drop them — the tracker uses `day_range()` = `{date}..{date+1}`. `dayType`/`dayName`/`nominalWorkTime` are not stored; the server rebuilds them from the user calendar, and `PUT` creates a missing day. The update answers `204 No Content` although the spec declares only `200`, so the client accepts any 2xx (`stored()`).
  - `work`: the start time of the running session (RFC 3339 text file, atomic write, mode 600, in-memory fallback without a path).
  - `wlan`: writes a managed block (`# BEGIN/END rolx-tracker`, hex SSID, derived PSK) to `/etc/wpa_supplicant.conf`, restarts with `/etc/init.d/S40network restart`. Only active with `linuxkms`.
  - `logging`: `tracing` to `/var/log/rolx-tracker.log`, 20 MB rotation, 10 files, stderr fallback.
- UI (`tracker/ui/app.slint`): left column with Sign in/Settings buttons filling the height, centre content, right status column. The network indicator is drawn last so dialogs do not cover it. Dialogs are overlays in the single window (KMS has no popups). Settings needs the admin password; the WLAN section shows only with `linuxkms`.
- The centre is the working screen: Start work / Stop work (local timestamp) and, after a stop, the activity list (`StandardListView`) plus Save. The sign-in dialog only holds the device flow.
- Keyboard: Tab/Shift-Tab/Enter/Escape work through `i-slint-core`'s focus traversal, so every interactive widget stays a std-widget; the widgets behind an open dialog get `enabled: root.dialog == "none"` so the focus cannot move out of the dialog (layouts have no `enabled`). Slint has no `%` operator (`Math.mod`) and `/` yields a float.
- The session is only kept in memory; the running work session is in `work_session_path`.

## Rolx server and Google sign-in

- `POST {server_url}/api/v1/SignIn` with `{"googleIdToken": ...}` returns `{user, bearerToken, expires}`; use `Authorization: Bearer <bearerToken>`. `GET /api/v1/signin/info` returns `{googleClientId, installationId}`.
- The server does not check the token's `aud`. Any Google OAuth client works if the user's hosted domain is whitelisted (m-f.ch, cudos.ch) and the account is confirmed. The reference CLI is the sibling project `../hackathon2026-rolxapp` (see its `docs/auth-plan.md`).
- The device flow must request the scope `openid email profile`. With only `openid email` the server answered HTTP 500. It also answers 500 with an empty body for any malformed token, so a 500 says little.
- Google's device flow cannot be bound to an account in advance, so the typed user name is only compared with the resulting e-mail.
- Google's verification page fills the code from `https://www.google.com/device?user_code=XXX-XXX-XXX` (undocumented, verified by fetching the page). The QR code uses it.
- `~/.config/rolx/config.toml` holds the shared server URL and the Google client credentials. `~/.config/rolx/tracker/config.toml` is the tracker's own config. Both contain secrets (mode 600): never print their values.
- In debug builds `device_flow` does not redact secrets in its debug logs; do not run with `RUST_LOG=debug` where logs are shared.

## Pitfalls

- `Cargo.lock` on `main` once contained committed merge-conflict markers; it was rebuilt from commit 14e56ff. Check for `<<<<<<<` after merges.
- UI changes cannot be checked by screenshot here (`import` fails on Wayland). Say that they are unverified and ask the user to look.

## Working agreements

- Never commit or push unless asked. Work on branch `eugen` (PRs are merged to `main`), not on `main`. End commit messages with the Co-Authored-By trailer given by the system reminder.
- Ask about hardware facts instead of guessing (the board restarts the network with `/etc/init.d/S40network restart`, wifi runs through wpa_supplicant). Make such values configurable with that default.
- For layout requests that are ambiguous, take the literal reading, say which one was chosen and offer the alternative.
- Keep summaries short and state what was and was not verified.
