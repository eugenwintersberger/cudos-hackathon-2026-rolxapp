use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
pub enum LoginFlow {
    /// Native app OAuth 2.0 flow with PKCE (default unless specified)
    NativeApp,
    /// Device authorization flow (for headless/SSH environments)
    Device {
        /// Show the URL, code and a QR code in a small window
        #[arg(long)]
        ui: bool,
    },
}

#[derive(Debug, Parser)]
#[command(
    name = "rolxapp",
    about = "Test executable for the device_flow library"
)]
pub struct Cli {
    /// Override the server URL from config
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Increase logging verbosity (-v for info, -vv for debug; can also be controlled via RUST_LOG)
    #[arg(long, short = 'v', global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sign in with Google and store session
    Login {
        /// OAuth flow to use (default: NativeApp)
        #[command(subcommand)]
        flow: Option<LoginFlow>,
    },
    /// Delete stored session
    Logout,
    /// Print current user from stored session
    Whoami,
    /// Inspect the config file
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// List the settings set in the config file
    Ls,
    /// Print the config file path only, e.g. for `$EDITOR "$(rolxapp config path)"`
    Path,
}
