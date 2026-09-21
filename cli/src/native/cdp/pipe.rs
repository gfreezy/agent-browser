//! Browser-level pipe used only to install the owned window helper. Older Chrome
//! requires a trusted pipe client for Extensions.loadUnpacked. Page commands
//! continue to use WebSocket CDP; keep this pipe alive until Chrome exits.
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, PipeReader, PipeWriter, Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::Duration;

pub(super) struct Endpoints {
    pub child_read: PipeReader,
    pub child_write: PipeWriter,
    parent_read: PipeReader,
    parent_write: PipeWriter,
}

impl Endpoints {
    pub fn new() -> io::Result<Self> {
        let (child_read, parent_write) = io::pipe()?;
        let (parent_read, child_write) = io::pipe()?;
        Ok(Self {
            child_read,
            child_write,
            parent_read,
            parent_write,
        })
    }

    #[cfg(unix)]
    pub fn configure(&self, command: &mut std::process::Command) -> io::Result<()> {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::os::unix::process::CommandExt;
        fn duplicate(fd: i32) -> io::Result<OwnedFd> {
            // Keep source descriptors above Chrome's reserved fd 3 and 4.
            let copied = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 5) };
            if copied < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(unsafe { OwnedFd::from_raw_fd(copied) })
        }
        let read = duplicate(self.child_read.as_raw_fd())?;
        let write = duplicate(self.child_write.as_raw_fd())?;
        // Only async-signal-safe syscalls run after fork. Command owns the
        // duplicates until spawn completes; dup2 clears CLOEXEC on fd 3/4.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(read.as_raw_fd(), 3) < 0 || libc::dup2(write.as_raw_fd(), 4) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Ok(())
    }

    pub fn connect(self) -> Transport {
        let Self {
            child_read,
            child_write,
            parent_read,
            parent_write,
        } = self;
        drop((child_read, child_write));
        let (sender, responses) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(parent_read);
            loop {
                match read_message(&mut reader) {
                    Ok(value) => {
                        // No domains are enabled on this connection. Ignore events.
                        if value.get("id").is_some() && sender.send(Ok(value)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            }
        });
        Transport {
            writer: parent_write,
            responses: Mutex::new(responses),
        }
    }
}

pub(super) struct Transport {
    writer: PipeWriter,
    responses: Mutex<Receiver<Result<Value, String>>>,
}

impl Transport {
    pub fn load_helper(&mut self, path: &Path) -> Result<String, String> {
        let request =
            json!({"id": 1, "method": "Extensions.loadUnpacked", "params": {"path": path}});
        let mut bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        bytes.push(0);
        self.writer
            .write_all(&bytes)
            .map_err(|e| format!("CDP pipe write failed: {e}"))?;
        let response = self
            .responses
            .get_mut()
            .map_err(|_| "CDP pipe response lock poisoned")?
            .recv_timeout(Duration::from_secs(15))
            .map_err(|e| format!("CDP pipe did not respond: {e}"))??;
        helper_id(response)
    }
}

fn helper_id(response: Value) -> Result<String, String> {
    if response["id"] != 1 {
        return Err("Unexpected CDP pipe response ID".into());
    }
    if let Some(error) = response.get("error") {
        return Err(format!(
            "Cannot load background window helper through CDP pipe: {error}"
        ));
    }
    response["result"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Missing window helper extension ID".into())
}

fn read_message(reader: &mut impl BufRead) -> Result<Value, String> {
    // Bound even malformed responses; messages are NUL-delimited, not newline-delimited.
    let mut bytes = Vec::new();
    reader
        .take(1024 * 1024 + 1)
        .read_until(0, &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("CDP pipe response exceeded 1 MiB".into());
    }
    if bytes.pop() != Some(0) {
        return Err("CDP pipe closed before a complete response".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid CDP pipe response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_fragmented_and_coalesced_messages() {
        let bytes = b"{\"id\":1,\"result\":{\"id\":\"extension\"}}\0{\"method\":\"event\"}\0";
        let mut reader = BufReader::with_capacity(2, &bytes[..]);
        assert_eq!(
            helper_id(read_message(&mut reader).unwrap()).unwrap(),
            "extension"
        );
        assert_eq!(read_message(&mut reader).unwrap()["method"], "event");
        assert!(read_message(&mut reader).is_err());
    }
    #[test]
    fn rejects_errors_missing_ids_and_truncated_messages() {
        assert!(
            helper_id(json!({"id":1,"error":{"message":"Method not available"}}))
                .unwrap_err()
                .contains("Method not available")
        );
        assert!(helper_id(json!({"id":1,"result":{}})).is_err());
        assert!(helper_id(json!({"id":2,"result":{"id":"x"}})).is_err());
        assert!(read_message(&mut &b"{}"[..]).is_err());
        assert!(read_message(&mut &vec![b'x'; 1024 * 1024 + 1][..]).is_err());
    }
}
