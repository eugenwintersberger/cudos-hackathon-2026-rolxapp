//! WLAN configuration for systems where wpa_supplicant manages the wifi.
//!
//! The network entered by the user is written to a block of the
//! wpa_supplicant configuration that is delimited by marker comments. Only
//! this block is replaced, everything else in the file is left alone.

use std::{
    fmt, fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::Command,
};

use pbkdf2::pbkdf2_hmac;
use sha1::Sha1;

const BEGIN: &str = "# BEGIN rolx-tracker";
const END: &str = "# END rolx-tracker";

/// Header of a configuration file that is created from scratch.
const NEW_FILE_HEADER: &str = "ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\n";

#[derive(Debug)]
pub enum Error {
    InvalidSsid,
    InvalidPassphrase,
    Io(String, io::Error),
    /// The command to restart the network failed.
    Restart(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSsid => f.write_str("the SSID must have 1 to 32 bytes"),
            Self::InvalidPassphrase => {
                f.write_str("the passphrase must have 8 to 63 printable ASCII characters")
            }
            Self::Io(path, e) => write!(f, "{path}: {e}"),
            Self::Restart(e) => write!(f, "restarting the network failed: {e}"),
        }
    }
}

impl std::error::Error for Error {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The 256 bit pre-shared key for a passphrase (IEEE 802.11i, annex H.4),
/// so that the passphrase itself does not have to be stored.
fn psk(ssid: &str, passphrase: &str) -> String {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha1>(passphrase.as_bytes(), ssid.as_bytes(), 4096, &mut key);
    hex(&key)
}

/// The `network` block for a WPA2 personal network.
fn network_block(ssid: &str, passphrase: &str) -> Result<String, Error> {
    if ssid.is_empty() || ssid.len() > 32 {
        return Err(Error::InvalidSsid);
    }
    if !(8..=63).contains(&passphrase.len()) || !passphrase.bytes().all(|b| (32..127).contains(&b))
    {
        return Err(Error::InvalidPassphrase);
    }
    // Both values are written as hex, so no input can break out of the block.
    Ok(format!(
        "{BEGIN}\nnetwork={{\n\tssid={}\n\tscan_ssid=1\n\tpsk={}\n\tkey_mgmt=WPA-PSK\n}}\n{END}\n",
        hex(ssid.as_bytes()),
        psk(ssid, passphrase),
    ))
}

/// `existing` with our network block replaced (or appended).
pub fn update_config(existing: &str, ssid: &str, passphrase: &str) -> Result<String, Error> {
    let block = network_block(ssid, passphrase)?;

    let mut config = String::new();
    let mut skipping = false;
    for line in existing.lines() {
        match line.trim() {
            BEGIN => skipping = true,
            END if skipping => skipping = false,
            _ if skipping => {}
            _ => {
                config.push_str(line);
                config.push('\n');
            }
        }
    }
    let config = config.trim_end();
    if config.is_empty() {
        return Ok(block);
    }
    Ok(format!("{config}\n\n{block}"))
}

/// Write `contents` to `path` atomically; the file holds a key, so it is
/// readable by its owner only.
fn write_atomically(path: &Path, contents: &str) -> Result<(), Error> {
    let io_error = |e| Error::Io(path.display().to_string(), e);
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = Path::new(&temporary);

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(temporary)
        .map_err(io_error)?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(temporary, path))
        .map_err(|e| {
            let _ = fs::remove_file(temporary);
            io_error(e)
        })
}

/// Store the network in the wpa_supplicant configuration at `config_path` and
/// run `restart_command` (program and arguments) to make it effective.
pub fn apply(
    config_path: &Path,
    restart_command: &[String],
    ssid: &str,
    passphrase: &str,
) -> Result<(), Error> {
    let existing = match fs::read_to_string(config_path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => NEW_FILE_HEADER.to_owned(),
        Err(e) => return Err(Error::Io(config_path.display().to_string(), e)),
    };
    write_atomically(config_path, &update_config(&existing, ssid, passphrase)?)?;

    let Some((program, args)) = restart_command.split_first() else {
        return Ok(());
    };
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| Error::Restart(format!("{program}: {e}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::Restart(format!("{program} exited with {}: {}", output.status, stderr.trim())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psk_matches_ieee_test_vector() {
        assert_eq!(
            psk("IEEE", "password"),
            "f42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e"
        );
    }

    #[test]
    fn validates_input() {
        assert!(matches!(network_block("", "password1"), Err(Error::InvalidSsid)));
        assert!(matches!(network_block(&"x".repeat(33), "password1"), Err(Error::InvalidSsid)));
        assert!(matches!(network_block("n", "short"), Err(Error::InvalidPassphrase)));
        assert!(matches!(network_block("n", &"x".repeat(64)), Err(Error::InvalidPassphrase)));
        assert!(matches!(network_block("n", "line\nbreak1"), Err(Error::InvalidPassphrase)));
    }

    #[test]
    fn block_hides_the_passphrase_and_cannot_be_injected() {
        let block = network_block("a\"\n}\nb", "secret-pass").unwrap();
        assert!(!block.contains("secret-pass"));
        assert_eq!(block.matches("network={").count(), 1);
        assert_eq!(block.lines().filter(|l| l.trim() == "}").count(), 1);
    }

    #[test]
    fn replaces_only_the_managed_block() {
        let first = update_config("country=CH\n", "one", "password1").unwrap();
        assert!(first.starts_with("country=CH\n\n# BEGIN rolx-tracker"));

        let second = update_config(&first, "two", "password2").unwrap();
        assert!(second.starts_with("country=CH\n"));
        assert_eq!(second.matches(BEGIN).count(), 1);
        assert!(second.contains(&hex(b"two")));
        assert!(!second.contains(&hex(b"one")));

        let with_other = format!("{second}\nnetwork={{\n\tssid=\"home\"\n}}\n");
        let third = update_config(&with_other, "three", "password3").unwrap();
        assert!(third.contains("ssid=\"home\""));
        assert!(third.contains(&hex(b"three")));
    }

    #[test]
    fn apply_writes_a_private_file_and_restarts() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("rolx-wlan-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wpa_supplicant.conf");

        let marker = dir.join("restarted");
        let restart = ["touch".to_owned(), marker.display().to_string()];
        apply(&path, &restart, "net", "password1").unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("ctrl_interface="));
        assert!(text.contains(&hex(b"net")));
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(marker.exists());

        let failing = ["false".to_owned()];
        assert!(matches!(
            apply(&path, &failing, "net", "password1"),
            Err(Error::Restart(_))
        ));
        fs::remove_dir_all(&dir).unwrap();
    }
}
