mod activities;
mod cli;
mod config_list;
mod logging;
mod prompt;
mod ui;

use clap::Parser;
use cli::{Cli, Command, ConfigCommand, LoginFlow};
use device_flow::{AppConfig, FileSessionStore};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    logging::init(cli.verbose);

    match cli.command {
        Command::Login { flow } => {
            let config = AppConfig::load()?.with_server_override(cli.server);
            let store = FileSessionStore::default_location()?;
            let session = match flow {
                Some(LoginFlow::NativeApp) | None => {
                    let pending = device_flow::login_browser(&config, &store).await?;
                    if let Some(url) = pending.manual_open_url() {
                        println!("{}", prompt::format_manual_open(url));
                    }
                    pending.finish(&store).await?
                }
                Some(LoginFlow::Device { ui }) => {
                    let pending = device_flow::login_code(&config).await?;
                    println!("{}", prompt::format_device_prompt(pending.prompt()));
                    if ui {
                        ui::login_with_window(pending, store).await?
                    } else {
                        pending.finish(&store).await?
                    }
                }
            };
            println!(
                "Logged in as {} {} <{}>",
                session.user_first_name, session.user_last_name, session.user_email
            );
        }
        Command::Logout => {
            device_flow::logout(&FileSessionStore::default_location()?)?;
            println!("Logged out.");
        }
        Command::Whoami => {
            let session = device_flow::whoami(&FileSessionStore::default_location()?)?;
            println!(
                "{} {} <{}>",
                session.user_first_name, session.user_last_name, session.user_email
            );
        }
        Command::Activities => {
            let config = AppConfig::load()?.with_server_override(cli.server);
            let store = FileSessionStore::default_location()?;
            let client = activities::authorized_client(&config, &store).await?;
            let today = chrono::Local::now().date_naive();
            let list = activities::fetch_activities(&client, today).await?;
            println!("{}", serde_json::to_string_pretty(&list)?);
        }
        Command::Config { command } => match command {
            ConfigCommand::Ls => {
                let config = AppConfig::load()?;
                for (key, value) in config_list::list(&config)? {
                    println!("{key} = {value}");
                }
            }
            ConfigCommand::Path => {
                println!("{}", device_flow::config_path()?.display());
            }
        },
    }
    Ok(())
}
