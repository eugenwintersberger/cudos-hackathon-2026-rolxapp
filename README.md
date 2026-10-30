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
