mod cli;
mod logging;

use clap::Parser;
use cli::{Cli, Command, ConfigCommand, LoginFlow};
use device_flow::{AppConfig, AuthFlow};

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
            let flow = match flow {
                Some(LoginFlow::NativeApp) | None => AuthFlow::NativeApp,
                Some(LoginFlow::Device) => AuthFlow::Device,
            };
            let config = AppConfig::load()?.with_server_override(cli.server);
            let session = device_flow::login(&config, flow).await?;
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
