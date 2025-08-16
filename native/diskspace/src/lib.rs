use rustler::{Atom, Binary, Env, NifResult, Term};
use std::ffi::OsStr;

// Unix-specific imports
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::io;
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

// Windows-specific imports
#[cfg(windows)]
use std::ptr;
#[cfg(windows)]
use std::ops::Deref;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{GetLastError, ERROR_SUCCESS};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetFileAttributesW, FILE_ATTRIBUTE_DIRECTORY, INVALID_FILE_ATTRIBUTES,
};
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::{
    FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
    FORMAT_MESSAGE_IGNORE_INSERTS,
};
#[cfg(windows)]
use windows::Win32::System::Memory::{GetProcessHeap, HeapFree, HEAP_FLAGS};

// nix imports with proper cfg to avoid unused warnings
#[cfg(all(unix, target_os = "linux"))]
use nix::sys::statfs::{statfs, Statfs};
#[cfg(all(unix, not(target_os = "linux")))]
use nix::sys::statvfs::{statvfs, Statvfs};

mod atoms {
    rustler::atoms! {
        ok,
        error,
        wrong_arity,
        invalid_path,
        alloc_failed,
        path_conversion_failed,
        not_directory,
        winapi_failed,
        statvfs_failed,
        statfs_failed,
        available,
        free,
        total,
        used,
        errno,
        errstr
    }
}

// Helper: Create {error, Reason} tuple
fn make_error_tuple<'a>(env: Env<'a>, reason: Atom) -> NifResult<Term<'a>> {
    Ok(rustler::types::tuple::make_tuple(
        env,
        &[atoms::error().to_term(env), reason.to_term(env)],
    ))
}

// Helper: Create {error, Reason, Detail} tuple
fn make_error_tuple3<'a>(env: Env<'a>, reason: Atom, detail: Term<'a>) -> NifResult<Term<'a>> {
    Ok(rustler::types::tuple::make_tuple(
        env,
        &[atoms::error().to_term(env), reason.to_term(env), detail],
    ))
}

#[cfg(unix)]
// Helper: Create error tuple with errno details
fn make_errno_error_tuple<'a>(env: Env<'a>, reason: Atom, err: io::Error) -> NifResult<Term<'a>> {
    let errnum = err.raw_os_error().unwrap_or(0);
    let errstr = err.to_string();
    let detail = rustler::types::map::map_new(env)
        .map_put(atoms::errno().to_term(env), errnum)?
        .map_put(atoms::errstr().to_term(env), errstr)?;
    make_error_tuple3(env, reason, detail)
}

#[cfg(windows)]
// Helper for safe WinAPI memory management
struct WinapiMessageBuffer(*mut u16);

#[cfg(windows)]
impl Deref for WinapiMessageBuffer {
    type Target = [u16];
    fn deref(&self) -> &Self::Target {
        // We can't know the size, so this is just for raw access.
        // The user must handle the length correctly.
        unsafe { std::slice::from_raw_parts(self.0, 0) }
    }
}

#[cfg(windows)]
impl Drop for WinapiMessageBuffer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            if let Ok(heap) = unsafe { GetProcessHeap() } {
                unsafe {
                    let _ = HeapFree(heap, HEAP_FLAGS(0), Some(self.0 as *const _));
                }
            }
        }
    }
}

#[cfg(windows)]
// Helper: Create error tuple with WinAPI error details
fn make_winapi_error_tuple<'a>(env: Env<'a>, reason: Atom, errnum: u32) -> NifResult<Term<'a>> {
    let mut buffer_ptr: *mut u16 = ptr::null_mut();
    let flags =
        FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS;
    let lang: u32 = 0; // Use system default

    let len = unsafe {
        FormatMessageW(
            flags,
            None,
            errnum,
            lang,
            PWSTR(&mut buffer_ptr as *mut _ as *mut _),
            0,
            None,
        )
    };

    let _buffer_guard = WinapiMessageBuffer(buffer_ptr);

    let errstr = if len == 0 || buffer_ptr.is_null() {
        "Unknown WinAPI error".to_string()
    } else {
        // This is safe because FormatMessageW guarantees null termination and length
        let slice = unsafe { std::slice::from_raw_parts(buffer_ptr, len as usize) };
        String::from_utf16_lossy(slice).trim().to_string()
    };

    let detail = rustler::types::map::map_new(env)
        .map_put(atoms::errno().to_term(env), errnum)?
        .map_put(atoms::errstr().to_term(env), errstr)?;
    make_error_tuple3(env, reason, detail)
}

/// Retrieves disk space information for a given path.
///
/// This NIF function takes a path, which can be either a `String` (list of characters)
/// or a `Binary`, and returns a tuple `{ok, map()}` containing disk space metrics,
/// or `{error, Reason}` if an error occurs.
///
/// The NIF schedules on a `DirtyIo` thread to prevent blocking the Erlang VM.
#[rustler::nif(schedule = "DirtyIo")]
fn stat_fs<'a>(env: Env<'a>, path_term: Term<'a>) -> NifResult<Term<'a>> {
    // Decode the path from the Elixir term.
    let path_bytes = match path_term.decode::<Binary>() {
        Ok(b) => b.to_vec(),
        Err(_) => {
            // Fallback to string (list of chars)
            let path_str: String = match path_term.decode() {
                Ok(s) => s,
                Err(_) => return make_error_tuple(env, atoms::invalid_path()),
            };
            path_str.into_bytes()
        }
    };
    if path_bytes.is_empty() {
        return make_error_tuple(env, atoms::invalid_path());
    }

    #[cfg(windows)]
    {
        // On Windows, paths are typically UTF-16. We first try to treat the
        // binary as UTF-8 for compatibility and convert to a wide string.
        let path_str = match String::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => return make_error_tuple(env, atoms::path_conversion_failed()),
        };

        // Standard Windows API calls fail with paths > 260 chars. The `\\?\` prefix
        // enables long path support and also simplifies UNC path handling.
        let long_path_str = if path_str.starts_with(r"\\?\") {
            path_str
        } else if path_str.starts_with(r"\\") {
            // Special case for UNC paths: \\server\share -> \\?\UNC\server\share
            format!(r"\\?\UNC{}", &path_str[1..])
        } else {
            format!(r"\\?\{}", path_str)
        };

        let wide_str = match widestring::WideCString::from_str(long_path_str) {
            Ok(ws) => ws,
            Err(_) => return make_error_tuple(env, atoms::path_conversion_failed()),
        };
        let wpath = PCWSTR::from_raw(wide_str.as_ptr());

        // Check if the path is a directory. GetDiskFreeSpaceExW will return volume info
        // for files, which is not what the test expects.
        let attr = unsafe { GetFileAttributesW(wpath) };
        if attr == INVALID_FILE_ATTRIBUTES {
            let err = unsafe { GetLastError() };
            return make_winapi_error_tuple(env, atoms::winapi_failed(), err);
        }
        if (attr & FILE_ATTRIBUTE_DIRECTORY.0) == 0 {
            return make_error_tuple(env, atoms::not_directory());
        }

        let mut avail: u64 = 0;
        let mut total: u64 = 0;
        let mut free: u64 = 0;
        let success = unsafe {
            GetDiskFreeSpaceExW(
                wpath,
                Some(&mut avail),
                Some(&mut total),
                Some(&mut free),
            )
        };

        // Note: GetDiskFreeSpaceExW returns FALSE on failure.
        if success == false {
            let err = unsafe { GetLastError() };
            // A common error for non-existent paths or non-directories is ERROR_INVALID_PARAMETER
            return make_winapi_error_tuple(env, atoms::winapi_failed(), err);
        }

        let used = total.saturating_sub(free);
        let map = rustler::types::map::map_new(env)
            .map_put(atoms::available().to_term(env), avail)?
            .map_put(atoms::free().to_term(env), free)?
            .map_put(atoms::total().to_term(env), total)?
            .map_put(atoms::used().to_term(env), used)?;
        Ok(rustler::types::tuple::make_tuple(
            env,
            &[atoms::ok().to_term(env), map],
        ))
    }

    #[cfg(unix)]
    {
        // On Unix, paths are byte sequences. We convert the binary to a Path.
        let path = Path::new(OsStr::from_bytes(&path_bytes));

        // Explicitly check if the path is a directory. statfs/statvfs may succeed
        // on a regular file and return the parent filesystem's stats.
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => return make_errno_error_tuple(env, atoms::not_directory(), e),
        };
        if !metadata.is_dir() {
            return make_error_tuple(env, atoms::not_directory());
        }

        #[cfg(target_os = "linux")]
        {
            let statfs_buf: Statfs = match statfs(path) {
                Ok(buf) => buf,
                Err(err) => {
                    let io_err = io::Error::from_raw_os_error(err as i32);
                    return make_errno_error_tuple(env, atoms::statfs_failed(), io_err);
                }
            };
            let block_size = statfs_buf.block_size() as u64;
            let avail = statfs_buf.blocks_available() as u64 * block_size;
            let free = statfs_buf.blocks_free() as u64 * block_size;
            let total = statfs_buf.blocks() as u64 * block_size;
            let used = total.saturating_sub(free);
            let map = rustler::types::map::map_new(env)
                .map_put(atoms::available().to_term(env), avail)?
                .map_put(atoms::free().to_term(env), free)?
                .map_put(atoms::total().to_term(env), total)?
                .map_put(atoms::used().to_term(env), used)?;
            Ok(rustler::types::tuple::make_tuple(
                env,
                &[atoms::ok().to_term(env), map],
            ))
        }

        #[cfg(not(target_os = "linux"))]
        {
            let statvfs_buf: Statvfs = match statvfs(path) {
                Ok(buf) => buf,
                Err(err) => {
                    let io_err = io::Error::from_raw_os_error(err as i32);
                    return make_errno_error_tuple(env, atoms::statvfs_failed(), io_err);
                }
            };
            let frag_size = statvfs_buf.fragment_size() as u64;
            let avail = statvfs_buf.blocks_available() as u64 * frag_size;
            let free = statvfs_buf.blocks_free() as u64 * frag_size;
            let total = statvfs_buf.blocks() as u64 * frag_size;
            let used = total.saturating_sub(free);
            let map = rustler::types::map::map_new(env)
                .map_put(atoms::available().to_term(env), avail)?
                .map_put(atoms::free().to_term(env), free)?
                .map_put(atoms::total().to_term(env), total)?
                .map_put(atoms::used().to_term(env), used)?;
            Ok(rustler::types::tuple::make_tuple(
                env,
                &[atoms::ok().to_term(env), map],
            ))
        }
    }
}

rustler::init!("Elixir.DiskSpace");