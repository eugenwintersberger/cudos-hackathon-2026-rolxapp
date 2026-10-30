mod cli;
mod logging;
mod prompt;

use clap::Parser;
use cli::{Cli, Command, ConfigCommand, LoginFlow};
use device_flow::AppConfig;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    logging::init(cli.verbose);

    match cli.command {
        Command::Login { flow } => {
            let config = AppConfig::load()?.with_server_override(cli.server);
            let session = match flow {
                Some(LoginFlow::NativeApp) | None => {
                    let pending = device_flow::login_browser(&config).await?;
                    if let Some(url) = pending.manual_open_url() {
                        println!("{}", prompt::format_manual_open(url));
                    }
                    pending.finish().await?
                }
                Some(LoginFlow::Device) => {
                    let pending = device_flow::login_code(&config).await?;
                    println!("{}", prompt::format_device_prompt(pending.prompt()));
                    pending.finish().await?
                }
            };
            println!(
                "Logged in as {} {} <{}>",
                session.user_first_name, session.user_last_name, session.user_email
            );
        }
        Command::Logout => {
            device_flow::logout()?;
            println!("Logged out.");
        }
        Command::Whoami => {
            let session = device_flow::whoami()?;
            println!(
                "{} {} <{}>",
                session.user_first_name, session.user_last_name, session.user_email
            );
        }
        Command::Config { command } => match command {
            ConfigCommand::Ls => {
                for (key, value) in device_flow::commands::config::list()? {
                    println!("{key} = {value}");
                }
            }
            ConfigCommand::Path => {
                println!("{}", device_flow::commands::config::path()?.display());
            }
        },
    }
    Ok(())
}
