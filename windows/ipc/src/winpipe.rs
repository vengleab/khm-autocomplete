//! Windows named-pipe transport. Windows only.
//!
//! Client (inside the TSF DLL): overlapped I/O with a hard deadline on every operation, so a stuck engine can
//! never hold a host app's UI thread. Server (inside `khengine.exe`): one pipe instance per client, with a DACL
//! that allows only the current user (plus app-container and low-integrity processes, which sandboxed apps are).
use super::TimedStream;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING, ERROR_PIPE_BUSY,
    HANDLE, HLOCAL, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    EqualSid, GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, WaitNamedPipeW, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, OpenProcess, OpenProcessToken, WaitForSingleObject,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn last_os_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { GetLastError() }.0 as i32)
}

/// The TOKEN_USER of a process token, as raw bytes (the SID inside points into this buffer).
fn token_user(token: HANDLE) -> io::Result<Vec<u8>> {
    unsafe {
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), len, &mut len)
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(buf)
    }
}

/// Is the process at the other end of this pipe running as the same user as we are? The pipe name is predictable,
/// so without this check another local user could create the pipe first and read what we send (the text before the caret).
fn verify_server_is_current_user(pipe: HANDLE) -> io::Result<()> {
    unsafe {
        let mut pid = 0u32;
        GetNamedPipeServerProcessId(pipe, &mut pid).map_err(|e| io::Error::other(e.to_string()))?;
        let process =
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(|e| io::Error::other(e.to_string()))?;
        let mut theirs = HANDLE::default();
        let r = OpenProcessToken(process, TOKEN_QUERY, &mut theirs);
        let _ = CloseHandle(process);
        r.map_err(|e| io::Error::other(e.to_string()))?;
        let their_user = token_user(theirs);
        let _ = CloseHandle(theirs);
        let mut mine = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut mine).map_err(|e| io::Error::other(e.to_string()))?;
        let my_user = token_user(mine);
        let _ = CloseHandle(mine);
        let (their_user, my_user) = (their_user?, my_user?);
        let same = EqualSid(
            (*(their_user.as_ptr() as *const TOKEN_USER)).User.Sid,
            (*(my_user.as_ptr() as *const TOKEN_USER)).User.Sid,
        )
        .is_ok();
        if same {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "the pipe belongs to another user"))
        }
    }
}

/// SID of the current user as a string such as `S-1-5-21-...`.
pub fn current_user_sid() -> io::Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(|e| io::Error::other(e.to_string()))?;
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let r = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), len, &mut len);
        let _ = CloseHandle(token);
        r.map_err(|e| io::Error::other(e.to_string()))?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut s).map_err(|e| io::Error::other(e.to_string()))?;
        let out = s.to_string().map_err(|e| io::Error::other(e.to_string()));
        let _ = LocalFree(Some(HLOCAL(s.0.cast())));
        out
    }
}

/// `\\.\pipe\KhmerAutocomplete.<user SID>`: one engine per user, no clash between users on a shared machine.
pub fn pipe_name() -> io::Result<String> {
    Ok(format!(r"\\.\pipe\KhmerAutocomplete.{}", current_user_sid()?))
}

// ------------------------------------------------------------------------------------------------ client

pub struct PipeStream {
    handle: HANDLE,
    event: HANDLE,
    deadline: Instant,
}

// the stream is only ever used from the thread that owns it
unsafe impl Send for PipeStream {}

impl Drop for PipeStream {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
            let _ = CloseHandle(self.event);
        }
    }
}

/// Connect to the engine's pipe within `timeout`. `NotFound` means the engine is not running.
pub fn connect(timeout: Duration) -> io::Result<PipeStream> {
    let name = wide(&pipe_name()?);
    let deadline = Instant::now() + timeout;
    loop {
        let h = unsafe {
            CreateFileW(
                PCWSTR(name.as_ptr()),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                // overlapped for deadlines; "identification" so the engine cannot impersonate us
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                None,
            )
        };
        match h {
            Ok(handle) => {
                // never send anything to a pipe that someone else created
                if let Err(e) = verify_server_is_current_user(handle) {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                    return Err(e);
                }
                let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
                    .map_err(|e| io::Error::other(e.to_string()))?;
                return Ok(PipeStream { handle, event, deadline });
            }
            Err(e) => {
                let code = e.code().0 as u32 & 0xFFFF;
                if code == ERROR_FILE_NOT_FOUND.0 {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "engine is not running"));
                }
                if code != ERROR_PIPE_BUSY.0 {
                    return Err(io::Error::other(e.to_string()));
                }
                // all instances busy: wait for one, but never past the deadline
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "engine busy"));
                }
                let _ = unsafe { WaitNamedPipeW(PCWSTR(name.as_ptr()), left.as_millis().min(u32::MAX as u128) as u32) };
            }
        }
    }
}

impl PipeStream {
    fn wait(&self, ov: &mut OVERLAPPED) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now()).as_millis().min(u32::MAX as u128) as u32;
        unsafe {
            if WaitForSingleObject(self.event, left) != WAIT_OBJECT_0 {
                // give up: cancel the operation and wait for the cancellation, so the buffer is not used afterwards
                let _ = CancelIoEx(self.handle, Some(ov));
                let mut n = 0u32;
                let _ = GetOverlappedResult(self.handle, ov, &mut n, true);
                return Err(io::Error::new(io::ErrorKind::TimedOut, "pipe operation timed out"));
            }
            let mut n = 0u32;
            GetOverlappedResult(self.handle, ov, &mut n, false).map_err(|_| {
                if GetLastError() == ERROR_BROKEN_PIPE {
                    io::Error::new(io::ErrorKind::BrokenPipe, "engine closed the pipe")
                } else {
                    last_os_error()
                }
            })?;
            Ok(n as usize)
        }
    }
}

impl Read for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut ov = OVERLAPPED { hEvent: self.event, ..Default::default() };
        unsafe {
            match ReadFile(self.handle, Some(buf), None, Some(&mut ov)) {
                Ok(()) => {
                    let mut n = 0u32;
                    GetOverlappedResult(self.handle, &ov, &mut n, false).map_err(|_| last_os_error())?;
                    Ok(n as usize)
                }
                Err(_) if GetLastError() == ERROR_IO_PENDING => self.wait(&mut ov),
                Err(_) if GetLastError() == ERROR_BROKEN_PIPE => Ok(0),
                Err(_) => Err(last_os_error()),
            }
        }
    }
}

impl Write for PipeStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut ov = OVERLAPPED { hEvent: self.event, ..Default::default() };
        unsafe {
            match WriteFile(self.handle, Some(buf), None, Some(&mut ov)) {
                Ok(()) => {
                    let mut n = 0u32;
                    GetOverlappedResult(self.handle, &ov, &mut n, false).map_err(|_| last_os_error())?;
                    Ok(n as usize)
                }
                Err(_) if GetLastError() == ERROR_IO_PENDING => self.wait(&mut ov),
                Err(_) => Err(last_os_error()),
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TimedStream for PipeStream {
    fn set_deadline(&mut self, deadline: Instant) -> io::Result<()> {
        self.deadline = deadline;
        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------ server

/// Owner (OW) gets everything; app-container processes (AC) and low-integrity processes (LW) may read and write,
/// so sandboxed apps can still use the keyboard. Nobody else, and no remote clients.
const PIPE_SDDL: &str = "D:(A;;GA;;;OW)(A;;GRGW;;;AC)S:(ML;;NW;;;LW)";

/// Create one more pipe instance and wait for a client to connect. Returns the connected pipe as a `File`
/// (blocking I/O is fine on the server's per-client thread). The very first instance (`first`) is created with
/// FILE_FLAG_FIRST_PIPE_INSTANCE, which fails if anyone else already owns this pipe name (pipe squatting).
pub fn accept(first: bool) -> io::Result<std::fs::File> {
    use std::os::windows::io::FromRawHandle;
    let name = wide(&pipe_name()?);
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(wide(PIPE_SDDL).as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
        .map_err(|e| io::Error::other(e.to_string()))?;
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: false.into(),
        };
        let h = CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            if first { FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_DUPLEX.0 | 0x0008_0000) } else { PIPE_ACCESS_DUPLEX },
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            super::MAX_FRAME as u32 + 4,
            super::MAX_FRAME as u32 + 4,
            0,
            Some(&sa),
        );
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        if h == INVALID_HANDLE_VALUE {
            return Err(last_os_error());
        }
        // a client that connected between CreateNamedPipe and ConnectNamedPipe is also fine (ERROR_PIPE_CONNECTED)
        if ConnectNamedPipe(h, None).is_err() && GetLastError().0 != 535 {
            let e = last_os_error();
            let _ = CloseHandle(h);
            return Err(e);
        }
        Ok(std::fs::File::from_raw_handle(h.0))
    }
}
