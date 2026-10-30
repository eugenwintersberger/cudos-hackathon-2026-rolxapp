use std::{env, error::Error, io};

fn main() -> Result<(), Box<dyn Error>> {
    if env::args().nth(1).as_deref() == Some("--hash-password") {
        // Print the hash for `admin_password_hash` in config.toml.
        let mut password = String::new();
        io::stdin().read_line(&mut password)?;
        println!("{}", tracker::config::hash_password(password.trim_end_matches(['\r', '\n']))?);
        return Ok(());
    }
    tracker::run()
}
