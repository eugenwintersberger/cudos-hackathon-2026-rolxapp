# Rolx Time Tracking App for small systems
Simple Rust app for time tracking with rolx

## Workspace layout

| Crate         | Type    | Purpose                                                        |
|---------------|---------|----------------------------------------------------------------|
| `tracker`     | lib+bin | The application. Slint UI; the library is reused by the board build. |
| `api`         | lib     | API library used by `tracker`.                                 |
| `device_flow` | lib     | Rolx sign-in (login, logout, whoami, config).                  |
| `device_flow_cli` | bin | `rolxapp` test executable for `device_flow`.                   |

## Building

Requires a recent Rust toolchain (edition 2024) and [rustup](https://rustup.rs).

### Linux desktop

```sh
cargo build --release          # whole workspace
cargo run -p tracker           # run the application
```

The `desktop` feature (winit + femtovg renderer) is enabled by default.

### RISC-V board (Linux KMS)

The board build imports the `tracker` library without the desktop backend and
enables the `linuxkms` feature (Slint `linuxkms-noseat` backend with the
software renderer):

```toml
tracker = { path = "../tracker", default-features = false, features = ["linuxkms"] }
```

To check that the feature compiles on a host:

```sh
cargo check -p tracker --no-default-features --features linuxkms --lib
```

This needs the development packages `libudev`, `libinput`, `libxkbcommon`,
`libdrm` and `libgbm` (for example `libudev-dev` on Debian). When
cross-compiling, they must be present in the target sysroot.

Add the RISC-V target and a matching linker, e.g.:

```sh
rustup target add riscv64gc-unknown-linux-gnu
```

At runtime the backend can be forced with `SLINT_BACKEND=linuxkms-noseat`.

## Configuration

The tracker reads `config.toml` from the first of these directories that
contains one (a file in `/etc` takes precedence over the user's):

1. `/etc/rolx/tracker/`
2. `$HOME/.config/rolx/tracker/`

| Key                   | Description                                                    |
|-----------------------|----------------------------------------------------------------|
| `admin_password_hash` | Argon2 hash of the administrator password. Without it the configuration dialog cannot be unlocked. |

| `server_url`          | Base URL of the Rolx service.                                  |
| `device_client_id`    | OIDC client id for the device flow login.                      |
| `device_client_secret`| OIDC client secret for the device flow login. Keep the file readable only by its user (`chmod 600`). |

The **Tracking** button needs the last three; without them it reports that the
login is not configured. Unknown keys are rejected. Create the hash with:

```sh
tracker --hash-password        # reads the password from stdin
```

and put the result into the file:

```toml
admin_password_hash = "$argon2id$v=19$m=19456,t=2,p=1$..."
```

The **Settings** button in the app asks for this password before it shows the
configuration dialog.

The **Tracking** button asks for the user name (e-mail address) and starts an
OIDC device flow: the app shows a link and a code, the user confirms the login
in any browser, and the app then signs in at the Rolx service. The account that
confirmed the login must match the user name that was entered.

## `device_flow` library API

Sign-in to Rolx via Google OAuth 2.0, plus session and config handling. The library never prints; the caller decides how to show prompts and results. Everything public is re-exported from the crate root.

**Login is two-phase.** Starting a login returns a *pending* login. Show the user what to do, then call `finish(&store)`, which signs in to Rolx and saves the session. Dropping a pending login cancels it.

```rust
use device_flow::{AppConfig, FileSessionStore, SessionStore};

async fn sign_in(cfg: &AppConfig, store: &impl SessionStore) -> device_flow::Result<()> {
    let pending = device_flow::login_code(cfg).await?;          // headless / SSH
    let p = pending.prompt();                                    // never contains secrets
    println!("Visit {} and enter {}", p.verification_uri, p.user_code);
    let session = pending.finish(store).await?;
    println!("Hello {}", session.user_first_name);
    Ok(())
}
```

| Item | Purpose |
|------|---------|
| `login_code(&cfg)` -> `PendingCodeLogin` | Device flow for devices without a browser. `prompt()` gives `DeviceCodePrompt` (URL, user code, expiry). Needs `google_device_client`. |
| `login_browser(&cfg, &store)` -> `PendingBrowserLogin` | Native app flow with PKCE; opens the local browser. `manual_open_url()` is `Some` if it could not. Needs `google_desktop_client`. Feature `browser-login`. |
| `pending.finish(&store)` | Waits for the user, signs in, saves and returns the `Session`. |
| `whoami(&store)` / `logout(&store)` | Load the stored `Session` (`Error::NoSession` if none) / delete it. Logout is local only. |
| `extend_session(&cfg, &store)` | Extends the stored session at Rolx and saves the result. |
| `Session` | Bearer token, expiry, user id, e-mail and name; `is_expired()`, `is_near_expiry()`. The token is a secret: do not log it. |
| `SessionStore` (trait) | `load() -> Option<Session>`, `save`, `delete`. Implement it for storage other than a file. |
| `FileSessionStore` | JSON file store: `default_location()` (`<state_dir>/rolx/session.json`) or `at(path)`. Feature `file-store`. |
| `AppConfig`, `ClientCredentials` | `server_url` and the optional Google client credentials. `AppConfig::load()` reads `<config_dir>/rolx/config.toml` (`config_path()`); or build it in code. |
| `Error`, `Result` | One `#[non_exhaustive]` error enum. Match on e.g. `NoSession`, `SessionExpired`, `AccessDenied`, `Expired`, `Unauthorized`, `MissingClientConfig`, `ConfigNotFound`. |

**Cargo features:** `file-store` and `browser-login` (both default), `uncensored_logs` (no log redaction, debug only). For a headless board use `default-features = false` and your own `SessionStore`.

The `device_flow_cli` crate (binary `rolxapp`) is an integration-test tool for this library: `cargo run -p device_flow_cli -- login [native-app|device]`, `logout`, `whoami`, `config ls|path`.