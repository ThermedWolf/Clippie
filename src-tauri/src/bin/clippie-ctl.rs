//! `clippie-ctl` — tiny trigger that talks to the running Clippie daemon.
//!
//! Hyprland binds exec this binary (`assets/hyprland/clippie-binds.lua`); it
//! sends one command to the daemon's control socket and exits immediately.
//! When the daemon is not running it fails fast with a non-zero exit — no
//! retry, no hang: a keypress must never stall the desktop.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Bounded so a wedged daemon can only delay the trigger briefly; a normal
/// reply is instantaneous.
const IO_TIMEOUT: Duration = Duration::from_secs(2);

const EXIT_USAGE: i32 = 1;
const EXIT_UNREACHABLE: i32 = 2;

fn main() {
    let Some(command) = parse_command(std::env::args().nth(1).as_deref()) else {
        eprintln!("usage: clippie-ctl <reveal|toggle|paste-1..9>");
        std::process::exit(EXIT_USAGE);
    };

    match send(&command) {
        Ok(reply) => println!("{reply}"),
        Err(error) => {
            eprintln!("clippie-ctl: {error}");
            std::process::exit(EXIT_UNREACHABLE);
        }
    }
}

/// Maps the CLI word to the wire command; unknown words are a usage error.
fn parse_command(argument: Option<&str>) -> Option<String> {
    match argument? {
        "reveal" => Some("REVEAL".into()),
        "toggle" => Some("TOGGLE".into()),
        word => {
            let digits = word.strip_prefix("paste-")?;
            let position: usize = digits.parse().ok()?;
            (1..=9).contains(&position).then(|| format!("PASTE {position}"))
        }
    }
}

/// Sends one command line and reads the daemon's single-line reply.
fn send(command: &str) -> Result<String, String> {
    let path = clippie_lib::control::socket_path();
    let stream = UnixStream::connect(&path).map_err(|error| {
        format!(
            "clippie daemon not reachable at {} ({error}); is it running?",
            path.display()
        )
    })?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("could not set socket timeout: {error}"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("could not set socket timeout: {error}"))?;

    let mut stream = stream;
    writeln!(stream, "{command}").map_err(|error| format!("could not send command: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("could not send command: {error}"))?;

    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|error| format!("no reply from daemon: {error}"))?;
    Ok(reply.trim_end().to_string())
}
