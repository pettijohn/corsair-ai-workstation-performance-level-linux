use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use corsair_performance_mode_core::Mode;
use nix::errno::Errno;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

pub const DEFAULT_CURRENT_MODE_PATH: &str =
    "/sys/bus/wmi/devices/99D89064-8D50-42BB-BEA9-155B2E5D0FCD/current_mode";

const RETRY_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    Mode(Mode),
    Unavailable(String),
}

pub fn spawn_watcher(path: PathBuf, tx: Sender<WatchEvent>) -> JoinHandle<()> {
    thread::spawn(move || watch_forever(path, tx))
}

fn watch_forever(path: PathBuf, tx: Sender<WatchEvent>) {
    let mut last_event: Option<WatchEvent> = None;

    loop {
        // Missing sysfs is normal before the driver is installed or loaded.
        // Keep the indicator alive and retry so autostart stays quiet.
        match OpenOptions::new().read(true).open(&path) {
            Ok(mut file) => {
                match read_mode(&mut file) {
                    Ok(mode) => send_changed(&tx, &mut last_event, WatchEvent::Mode(mode)),
                    Err(err) => {
                        send_changed(&tx, &mut last_event, read_error_event(&path, &err));
                        sleep_or_stop(&tx);
                        continue;
                    }
                }

                if watch_open_file(&path, &mut file, &tx, &mut last_event).is_err() {
                    break;
                }
            }
            Err(err) => {
                send_changed(&tx, &mut last_event, open_error_event(&path, &err));
                sleep_or_stop(&tx);
            }
        }
    }
}

fn watch_open_file(
    path: &Path,
    file: &mut File,
    tx: &Sender<WatchEvent>,
    last_event: &mut Option<WatchEvent>,
) -> Result<(), ()> {
    loop {
        // sysfs_notify() wakes poll with urgent/error readiness. This is not
        // timer polling: the thread sleeps here until the kernel reports a
        // mode attribute change.
        let mut fds = [PollFd::new(
            file.as_fd(),
            PollFlags::POLLPRI | PollFlags::POLLERR,
        )];

        match poll(&mut fds, PollTimeout::NONE) {
            Ok(_) => match read_mode(file) {
                Ok(mode) => send_changed(tx, last_event, WatchEvent::Mode(mode)),
                Err(err) => {
                    send_changed(tx, last_event, read_error_event(path, &err));
                    sleep_or_stop(tx);
                    return Ok(());
                }
            },
            Err(Errno::EINTR) => continue,
            Err(err) => {
                send_changed(
                    tx,
                    last_event,
                    WatchEvent::Unavailable(format!("Mode watcher failed: {err}")),
                );
                sleep_or_stop(tx);
                return Ok(());
            }
        }
    }
}

fn read_mode(file: &mut File) -> io::Result<Mode> {
    let mut value = String::new();
    // sysfs attributes behave like generated files; rereads after poll must
    // rewind to offset 0 or they can return an empty string.
    file.seek(SeekFrom::Start(0))?;
    file.read_to_string(&mut value)?;
    Ok(Mode::from_sysfs_value(&value))
}

fn send_changed(tx: &Sender<WatchEvent>, last_event: &mut Option<WatchEvent>, event: WatchEvent) {
    if last_event.as_ref() == Some(&event) {
        return;
    }

    *last_event = Some(event.clone());
    let _ = tx.send(event);
}

fn open_error_event(path: &Path, err: &io::Error) -> WatchEvent {
    if err.kind() == io::ErrorKind::NotFound {
        WatchEvent::Unavailable("Kernel driver may be missing".to_string())
    } else {
        WatchEvent::Unavailable(format!("Cannot open {}: {err}", path.display()))
    }
}

fn read_error_event(path: &Path, err: &io::Error) -> WatchEvent {
    WatchEvent::Unavailable(format!("Cannot read {}: {err}", path.display()))
}

fn sleep_or_stop<T>(tx: &Sender<T>) {
    let _ = tx;
    thread::sleep(RETRY_DELAY);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_mode_from_file_start_each_time() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "corsair-mode-indicator-test-{}",
            std::process::id()
        ));

        {
            let mut file = File::create(&path).unwrap();
            writeln!(file, "balanced").unwrap();
        }

        let mut file = OpenOptions::new().read(true).open(&path).unwrap();
        assert_eq!(read_mode(&mut file).unwrap(), Mode::Balanced);
        assert_eq!(read_mode(&mut file).unwrap(), Mode::Balanced);

        let _ = std::fs::remove_file(path);
    }
}
