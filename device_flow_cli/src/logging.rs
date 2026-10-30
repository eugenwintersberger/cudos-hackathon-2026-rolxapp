use tracing_subscriber::EnvFilter;

pub fn init(verbosity: u8) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match verbosity {
            0 => "warn",
            1 => "info",
            _ => "debug",
        };

        EnvFilter::new(level)
    });

    // Logs go to stderr, so stdout stays clean for output like `config path`.
    if let Err(err) = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
    {
        eprintln!("failed to initialize logging: {err}");
    }
}
