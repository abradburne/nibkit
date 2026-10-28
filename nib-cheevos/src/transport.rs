//! The service's requests, carried by `curl`: nib-cheevos has no TLS of its own, and nibOS's
//! tools use `curl` the same way. The form data (which carries the password or the token) goes
//! to `curl` on its standard input, never in an argument.
//!
//! [`perform`] makes one request and waits for it: for the launcher, on a thread of its own.
//! [`start`] is the emulator's network thread: the emulation thread hands a request over and
//! goes on, and the answer comes back on a channel and is given to rcheevos on the next frame.
//!
//! When `curl` fails, it is told apart ([`Failure`]): a request that could never be made
//! (`curl` said the address or an argument was wrong) is not worth asking again, and rcheevos is
//! told so; anything else, the network down, the service slow, a certificate the clock is not
//! yet right for, `curl` missing for a moment, may work later, and rcheevos asks again, holding
//! an unlock until it is sent.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(feature = "client")]
use std::sync::mpsc::{Receiver, Sender};

/// How long one request may take, in all (the service answers in well under a second; a
/// stalled network should not hold a thread for long, and rcheevos retries).
const TIMEOUT_SECS: u32 = 30;
const CONNECT_TIMEOUT_SECS: u32 = 10;

/// The `curl` to talk to the service with: the first on `PATH`, else `/usr/bin/curl`.
pub fn curl_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain([PathBuf::from("/usr/bin")])
        .map(|dir| dir.join("curl"))
        .find(|p| p.is_file())
}

/// A request: where, and the form to POST (its content type, then the form), `None` for a GET.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The address, query and all.
    pub url: String,
    /// `(content type, form)` to POST; `None` for a GET.
    pub post: Option<(String, String)>,
}

/// Why a request got no answer from the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Asking again may work: anything but a request `curl` could never make.
    pub retry: bool,
    /// The network is down or the service unreachable: no name, no connection, no answer in
    /// time.
    pub offline: bool,
    /// What `curl` said.
    pub why: String,
}

impl Failure {
    /// `curl`'s exit code, and what it said, as a failure.
    fn of_exit(code: Option<i32>, stderr: &str) -> Failure {
        let why = stderr.lines().next().unwrap_or("").trim();
        let why = match (why.is_empty(), code) {
            (false, _) => why.to_owned(),
            (true, Some(c)) => format!("curl exited with {c}"),
            (true, None) => "curl was killed".to_owned(),
        };
        Failure {
            // Unsupported protocol, a malformed address, a bad argument: the request itself.
            retry: !matches!(code, Some(1 | 3 | 43)),
            // Couldn't resolve, couldn't connect, timed out, the TLS connection failed.
            offline: matches!(code, Some(6 | 7 | 28 | 35)),
            why,
        }
    }

    /// `curl` could not be run at all: worth trying again (it may be back), not offline.
    fn not_run(why: String) -> Failure {
        Failure {
            retry: true,
            offline: false,
            why,
        }
    }

    /// The HTTP status to give rcheevos for it: its own two numbers for "never reached".
    #[cfg(feature = "client")]
    pub(crate) fn status(&self) -> i32 {
        if self.retry {
            crate::ffi::NOT_REACHED
        } else {
            crate::ffi::CANNOT_ASK
        }
    }
}

/// `curl` as every request runs it: quiet but for errors, time limits, the User-Agent.
fn curl(curl: &Path, user_agent: &str) -> Command {
    let mut cmd = Command::new(curl);
    cmd.args([
        "-sS",
        "--max-time",
        &TIMEOUT_SECS.to_string(),
        "--connect-timeout",
        &CONNECT_TIMEOUT_SECS.to_string(),
        "-A",
        user_agent,
    ]);
    cmd
}

/// Runs `curl` for one request: the HTTP status and the body, or why the service wasn't
/// reached.
pub fn perform(
    curl_at: &Path,
    request: &Request,
    user_agent: &str,
) -> Result<(i32, Vec<u8>), Failure> {
    let mut cmd = curl(curl_at, user_agent);
    cmd.args(["--compressed", "-o", "-", "-w", "\n%{http_code}"]);
    if let Some((content_type, _)) = &request.post {
        cmd.args(["-H", &format!("Content-Type: {content_type}")]);
        // No 100-continue wait.
        cmd.args(["-H", "Expect:", "--data-binary", "@-"]);
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    cmd.arg(&request.url);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| Failure::not_run(format!("curl: {e}")))?;
    if let (Some((_, form)), Some(mut stdin)) = (&request.post, child.stdin.take()) {
        let _ = stdin.write_all(form.as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|e| Failure::not_run(format!("curl: {e}")))?;
    if !out.status.success() {
        return Err(Failure::of_exit(
            out.status.code(),
            &String::from_utf8_lossy(&out.stderr),
        ));
    }
    match status_and_body(out.stdout) {
        // A transfer that ended without a status line: try again.
        (_, 0) => Err(Failure::not_run("curl gave no HTTP status".into())),
        (body, status) => Ok((status, body)),
    }
}

/// Fetches `url` into the file `to`, whole or not at all: into a file beside it, then renamed
/// over it, so a half-fetched file is never where a whole one is looked for. An HTTP error (a
/// badge the service hasn't got) fails.
pub fn fetch(curl_at: &Path, url: &str, to: &Path, user_agent: &str) -> Result<(), Failure> {
    let dir = to.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)
        .map_err(|e| Failure::not_run(format!("{}: {e}", dir.display())))?;
    let name = to.file_name().unwrap_or_default().to_string_lossy();
    let part = to.with_file_name(format!(".{name}.part"));
    let out = curl(curl_at, user_agent)
        .args(["-f", "-o"])
        .arg(&part)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| Failure::not_run(format!("curl: {e}")))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(Failure::of_exit(
            out.status.code(),
            &String::from_utf8_lossy(&out.stderr),
        ));
    }
    std::fs::rename(&part, to).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        Failure::not_run(format!("{}: {e}", to.display()))
    })
}

/// The body and the status `curl` wrote after it on a line of its own.
fn status_and_body(mut stdout: Vec<u8>) -> (Vec<u8>, i32) {
    let Some(nl) = stdout.iter().rposition(|&b| b == b'\n') else {
        return (stdout, 0);
    };
    let status = std::str::from_utf8(&stdout[nl + 1..])
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    stdout.truncate(nl);
    (stdout, status)
}

/// A request rcheevos made, with the ticket that answers it.
#[cfg(feature = "client")]
pub(crate) struct Job {
    pub request: Request,
    pub ticket: usize,
}

/// An answer: the HTTP status and the body, or, when the service was never reached, rcheevos'
/// number for that ([`Failure::status`]) and why, which rcheevos logs (and retries, when it may).
#[cfg(feature = "client")]
pub(crate) struct Done {
    pub ticket: usize,
    pub status: i32,
    pub body: Vec<u8>,
}

#[cfg(feature = "client")]
impl Done {
    /// The service wasn't reached, for this reason.
    pub fn failed(ticket: usize, failure: &Failure) -> Done {
        Done {
            ticket,
            status: failure.status(),
            body: failure.why.as_bytes().to_vec(),
        }
    }
}

/// Starts the network thread, which runs `curl` for each job that comes in and sends its
/// answer out. It ends when the jobs' sender is dropped. Without a thread the jobs go nowhere;
/// the client answers them itself.
#[cfg(feature = "client")]
pub(crate) fn start(
    curl: PathBuf,
    user_agent: String,
    incoming: Receiver<Job>,
    outgoing: Sender<Done>,
) {
    let spawned = std::thread::Builder::new()
        .name("cheevos-net".into())
        .spawn(move || {
            ordinary_priority();
            for job in incoming {
                let done = match perform(&curl, &job.request, &user_agent) {
                    Ok((status, body)) => Done {
                        ticket: job.ticket,
                        status,
                        body,
                    },
                    Err(failure) => Done::failed(job.ticket, &failure),
                };
                if outgoing.send(done).is_err() {
                    break;
                }
            }
        });
    if let Err(e) = spawned {
        eprintln!("cheevos: cannot start the network thread: {e}");
    }
}

/// A thread that waits on the network must not hold the emulation thread's scheduling
/// (a real-time class, or its utilization clamp, where the launcher gave it one).
#[cfg(feature = "client")]
pub(crate) fn ordinary_priority() {
    #[cfg(target_os = "linux")]
    {
        let param = libc::sched_param { sched_priority: 0 };
        // SAFETY: this thread, an ordinary policy, a fully initialised param.
        unsafe {
            libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_OTHER, &param);
            libc::nice(10);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_follows_the_body() {
        assert_eq!(
            status_and_body(b"{\"Success\":true}\n200".to_vec()),
            (b"{\"Success\":true}".to_vec(), 200)
        );
        assert_eq!(
            status_and_body(b"body\n\n404".to_vec()),
            (b"body\n".to_vec(), 404)
        );
        assert_eq!(status_and_body(b"".to_vec()), (Vec::new(), 0));
        assert_eq!(status_and_body(b"\n000".to_vec()), (Vec::new(), 0));
    }

    /// A request that can never be made is not asked again; the network down is, and is
    /// offline; anything else is asked again but is no sign of the network.
    #[test]
    fn curl_failing_is_retried_unless_the_request_itself_is_wrong() {
        let f = Failure::of_exit(Some(6), "curl: (6) Could not resolve host: x\n");
        assert_eq!((f.retry, f.offline), (true, true));
        assert_eq!(f.why, "curl: (6) Could not resolve host: x");
        for code in [7, 28, 35] {
            assert!(Failure::of_exit(Some(code), "").offline, "{code}");
        }
        let cert = Failure::of_exit(Some(60), "curl: (60) SSL certificate problem");
        assert_eq!((cert.retry, cert.offline), (true, false));
        for code in [1, 3, 43] {
            assert!(!Failure::of_exit(Some(code), "").retry, "{code}");
        }
        assert_eq!(Failure::of_exit(Some(22), "").why, "curl exited with 22");
        assert_eq!(Failure::of_exit(None, "").why, "curl was killed");
    }

    #[cfg(feature = "client")]
    #[test]
    fn rcheevos_hears_whether_to_ask_again() {
        let again = Failure::of_exit(Some(7), "");
        let never = Failure::of_exit(Some(3), "");
        assert_eq!(again.status(), crate::ffi::NOT_REACHED);
        assert_eq!(never.status(), crate::ffi::CANNOT_ASK);
        let d = Done::failed(9, &never);
        assert_eq!((d.ticket, d.status), (9, crate::ffi::CANNOT_ASK));
    }
}
