# Rolx Time Tracking App for small systems
Simple Rust app for time tracking with rolx

## Workspace layout

| Crate         | Type    | Purpose                                                        |
|---------------|---------|----------------------------------------------------------------|
| `tracker`     | lib+bin | The application. Slint UI; the library is reused by the board build. |
| `api`         | lib     | API library used by `tracker`.                                 |
| `device_flow` | bin     | Device flow tool.                                              |

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
