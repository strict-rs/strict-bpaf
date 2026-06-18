mod vterm;

use ptyprocess::{PtyProcess, Signal};
use std::{
    io::{Read, Write},
    process::Command,
    time::Duration,
};
use vterm::Term;

pub const WIDTH: u16 = 120;
pub const HEIGHT: u16 = 60;
pub const ZSH_TIMEOUT: Duration = Duration::from_millis(100);
pub const BASH_TIMEOUT: Duration = Duration::from_millis(50);
pub const FISH_TIMEOUT: Duration = Duration::from_millis(100);
pub const ELVISH_TIMEOUT: Duration = Duration::from_millis(50);
/// Upper bound on waiting for a shell's first byte in a phase (startup, or a completion's first
/// output). Generous enough never to race a slow start, but finite so a silent shell can't hang.
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(5);

/// Do zsh completion test for this input
///
/// if `print` is `true` - print raw output and exit
pub fn zsh_comptest(input: &str) -> anyhow::Result<String> {
    zsh_comptest_with(input, 120)
}

pub fn zsh_comptest_with(input: &str, width: u16) -> anyhow::Result<String> {
    let cwd = std::env::current_dir()?;
    let repo = cwd.parent().unwrap();
    let path = format!(
        "{}:{}/target/release/examples",
        std::env::var("PATH")?,
        repo.display()
    );
    // `compinit` refuses to autoload completion functions from group- or world-writable
    // directories, and the checkout is group-writable. Stage them into a private 0700 directory so
    // `compinit`'s `compaudit` security check passes.
    let zdotdir = secure_zsh_zdotdir(repo)?;
    let mut command = Command::new("zsh");
    command.env("PATH", path).env("ZDOTDIR", &zdotdir);
    let result = comptest(command, false, input, width, ZSH_TIMEOUT, false);
    let _ = std::fs::remove_dir_all(&zdotdir);
    result
}

/// Stage the zsh completion functions into a fresh owner-only (`0700`) directory and return it for
/// use as `ZDOTDIR`. The source `dotfiles/zsh` inherits the checkout's group-writable permissions,
/// which `compinit`'s `compaudit` rejects; a private copy is secure regardless of how the repository
/// was checked out. The caller removes the directory when finished.
fn secure_zsh_zdotdir(repo: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique = format!(
        "bpaf-comptest-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let zdotdir = std::env::temp_dir().join(unique);
    let fpath_dir = zdotdir.join("zsh");
    let _ = std::fs::remove_dir_all(&zdotdir);
    std::fs::create_dir_all(&fpath_dir)?;
    std::fs::set_permissions(&zdotdir, std::fs::Permissions::from_mode(0o700))?;
    std::fs::set_permissions(&fpath_dir, std::fs::Permissions::from_mode(0o700))?;
    for entry in std::fs::read_dir(repo.join("dotfiles/zsh"))? {
        let entry = entry?;
        std::fs::copy(entry.path(), fpath_dir.join(entry.file_name()))?;
    }
    std::fs::write(
        zdotdir.join(".zshenv"),
        format!(
            "fpath=($fpath {})\nautoload -U +X compinit && compinit\nPS1='%% '\n",
            fpath_dir.display()
        ),
    )?;
    Ok(zdotdir)
}

pub fn bash_comptest(input: &str) -> anyhow::Result<String> {
    let cwd = std::env::current_dir()?;
    let cwd = cwd.parent().unwrap().to_str().unwrap();
    let path = format!("{}:{cwd}/target/release/examples", std::env::var("PATH")?,);
    let mut command = Command::new("bash");
    command
        .env("PATH", path)
        // Interactive bash sources the host's system-wide rc (e.g. /etc/bash.bashrc) before our
        // --rcfile. On hosts that wire a build-environment profile in there, its `pyenv init`
        // ends in a `pyenv rehash` that intermittently prints `find: …/.pyenv-shim: No such file
        // or directory` onto the terminal, corrupting the scraped completion. That profile skips
        // re-initialization when `_BUILDENV_LOADED` is already set, so mark it loaded to keep
        // pyenv out of the screen; the `_filedir` helper bpaf's bash completion needs is sourced
        // independently and stays available.
        .env("_BUILDENV_LOADED", "1")
        .args(["--rcfile", &format!("{cwd}/dotfiles/.bashrc")]);
    let echo = !input.contains("\t\t");
    comptest(command, echo, input, 120, BASH_TIMEOUT, false)
}

pub fn fish_comptest(input: &str) -> anyhow::Result<String> {
    let cwd = std::env::current_dir()?;
    let cwd = cwd.parent().unwrap().to_str().unwrap();
    let path = format!("{}:{cwd}/target/release/examples", std::env::var("PATH")?,);
    let mut command = Command::new("fish");
    command
        .env("PATH", path)
        .env("XDG_CONFIG_HOME", format!("{cwd}/dotfiles"));
    comptest(command, false, input, 120, FISH_TIMEOUT, true)
}

pub fn elvish_comptest(input: &str) -> anyhow::Result<String> {
    let cwd = std::env::current_dir()?;
    let cwd = cwd.parent().unwrap().to_str().unwrap();
    let path = format!("{}:{cwd}/target/release/examples", std::env::var("PATH")?,);
    let mut command = Command::new("elvish");
    command
        .env("PATH", path)
        .env("XDG_CONFIG_HOME", format!("{cwd}/dotfiles"));
    comptest(command, false, input, 120, ELVISH_TIMEOUT, false)
}

fn comptest(
    command: Command,
    echo: bool,
    input: &str,
    width: u16,
    timeout: Duration,
    force_kill: bool,
) -> anyhow::Result<String> {
    // Spawn the shell under a real pty. Loading the completion machinery (sourcing rc files,
    // defining completion functions) takes a while, so the shell needs room to settle both
    // before we drive it and after.
    let mut process = PtyProcess::spawn(command)?;
    process.set_window_size(width, HEIGHT)?;
    // for some reason bash does not produce anything with echo disabled...
    process.set_echo(echo, None)?;

    let mut term = Term::new(width, HEIGHT);
    let mut stream = process.get_raw_handle()?;

    // Read the pty on a dedicated thread so the orchestration below can use channel timeouts
    // to detect "the shell went quiet" instead of being stuck in a blocking read.
    let (snd, rcv) = std::sync::mpsc::channel::<Vec<u8>>();
    let mut reader_fd = stream.try_clone()?;
    let reader = std::thread::spawn(move || {
        let mut buf = [0; 2048];
        while let Ok(n) = reader_fd.read(&mut buf) {
            if n == 0 || snd.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    // Let the shell finish starting and settle at its prompt before sending anything. With echo
    // on, the tty echoes our input the instant it is written; if that lands while the shell is
    // still starting up, the idle detector would trip on the echo and the shell would be killed
    // before it ever reaches a prompt. `drain_until_idle` blocks for the first byte, so however
    // long startup takes is absorbed here rather than raced.
    drain_until_idle(&rcv, &mut term, &mut stream, timeout);

    // Drive the completion.
    write!(stream, "{}", input)?;
    stream.flush()?;

    // Capture until the shell goes quiet again.
    drain_until_idle(&rcv, &mut term, &mut stream, timeout);

    // fish erases its completion pager on any catchable signal (it emits `\r\n\x1b[J` during
    // shutdown), so it must be SIGKILLed to freeze the screen exactly as the user sees it. bash,
    // by contrast, flushes its completion during the graceful exit's inter-signal delays, so an
    // outright kill drops it — hence the per-shell choice.
    if force_kill {
        let _ = process.kill(Signal::SIGKILL);
    } else {
        let _ = process.exit(false);
    }
    let _ = reader.join();

    Ok(term.render())
}

/// Feed pty output to the emulator until the shell stays silent for `timeout`.
///
/// The first chunk is awaited without a deadline — a shell can take arbitrarily long to start —
/// after which a `timeout`-long silence is taken to mean "the shell is done for now". Capability
/// probes the shell emits (fish's OSC 11 / XTVERSION / DA1, …) are answered as they arrive, the
/// way a real terminal would, or fish stalls waiting for the replies and never renders.
fn drain_until_idle(
    rcv: &std::sync::mpsc::Receiver<Vec<u8>>,
    term: &mut Term,
    stream: &mut std::fs::File,
    timeout: Duration,
) {
    // Wait generously for the first byte — a shell can take a moment to start, and an echo-off
    // completion that resolves to a unique match emits nothing at all — but never wait forever,
    // or a silent shell would hang the test. The cap is far longer than any real shell startup.
    match rcv.recv_timeout(FIRST_BYTE_TIMEOUT) {
        Ok(chunk) => feed(&chunk, term, stream),
        Err(_) => return,
    }
    while let Ok(chunk) = rcv.recv_timeout(timeout) {
        feed(&chunk, term, stream);
    }
}

/// Answer any terminal probes in `chunk` and feed it to the emulator.
fn feed(chunk: &[u8], term: &mut Term, stream: &mut std::fs::File) {
    let reply = terminal_reply(chunk);
    if !reply.is_empty() {
        let _ = stream.write_all(&reply);
        let _ = stream.flush();
    }
    term.process(chunk);
}

/// Reply to the terminal capability probes a shell sends on startup, the way a real terminal would.
///
/// Modern fish interrogates the terminal — background color (OSC 11), version (XTVERSION), terminfo
/// capabilities (XTGETTCAP), the kitty keyboard protocol, and finally Primary Device Attributes
/// (DA1) as a synchronization barrier — and then blocks until the replies arrive. There is no real
/// terminal behind this scraper, so without answers fish never gets past the probe and renders
/// nothing. zsh, bash and elvish send none of these, so this is a no-op for them.
fn terminal_reply(out: &[u8]) -> Vec<u8> {
    let mut reply = Vec::new();
    // OSC 11 — background color query
    if find(out, b"\x1b]11;?").is_some() {
        reply.extend_from_slice(b"\x1b]11;rgb:0000/0000/0000\x1b\\");
    }
    // XTVERSION
    if find(out, b"\x1b[>0q").is_some() {
        reply.extend_from_slice(b"\x1bP>|comptester(0)\x1b\\");
    }
    // XTGETTCAP — report every requested capability as unknown so fish falls back to its defaults
    let mut rest = out;
    while let Some(start) = find(rest, b"\x1bP+q") {
        let after = &rest[start + 4..];
        let Some(end) = find(after, b"\x1b\\") else {
            break;
        };
        reply.extend_from_slice(b"\x1bP0+r");
        reply.extend_from_slice(&after[..end]);
        reply.extend_from_slice(b"\x1b\\");
        rest = &after[end + 2..];
    }
    // kitty keyboard protocol flags query — report no support
    if find(out, b"\x1b[?u").is_some() {
        reply.extend_from_slice(b"\x1b[?0u");
    }
    // Primary Device Attributes (the sync barrier) — answer last, as a VT102-class terminal
    if find(out, b"\x1b[0c").is_some() || find(out, b"\x1b[c").is_some() {
        reply.extend_from_slice(b"\x1b[?6c");
    }
    reply
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
