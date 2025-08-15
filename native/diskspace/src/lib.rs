use rustler::{Atom, Binary, Env, Error, NifResult, Term};
use std::ffi::{CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::{io, mem};

#[cfg(windows)]
use widestring::WideCString;
#[cfg(windows)]
use winapi::um::fileapi::{GetDiskFreeSpaceExW, GetFileAttributesW};
#[cfg(windows)]
use winapi::um::winnt::{FILE_ATTRIBUTE_DIRECTORY, INVALID_FILE_ATTRIBUTES};

#[cfg(target_os = "linux")]
use libc::statfs64;
#[cfg(not(target_os = "linux"))]
use libc::statvfs64;

mod atoms {
    rustler::atoms! {
        error,
        ok,
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
fn make_error_tuple<'a>(env: Env<'a>, reason: &'static str) -> NifResult<Term<'a>> {
    let error_atom = atoms::error();
    let reason_atom = Atom::from_str(env, reason)?;
    // Use to_term() instead of term()
    Ok(rustler::types::tuple::make_tuple(
        env,
        &[error_atom.to_term(env), reason_atom.to_term(env)],
    ))
}

// Helper: Create {error, Reason, Detail} tuple
fn make_error_tuple3<'a>(
    env: Env<'a>,
    reason: &'static str,
    detail: Term<'a>,
) -> NifResult<Term<'a>> {
    let error_atom = atoms::error();
    let reason_atom = Atom::from_str(env, reason)?;
    // Use to_term() instead of term()
    Ok(rustler::types::tuple::make_tuple(
        env,
        &[error_atom.to_term(env), reason_atom.to_term(env), detail],
    ))
}

// Helper: Create error tuple with errno details
fn make_errno_error_tuple<'a>(
    env: Env<'a>,
    reason: &'static str,
    err: io::Error,
) -> NifResult<Term<'a>> {
    let errnum = err.raw_os_error().unwrap_or(0);
    let errstr = err.to_string();
    let detail = rustler::types::map::map_new(env)
        // Use to_term() instead of term()
        .map_put(atoms::errno().to_term(env), errnum)?
        // Use to_term() instead of term()
        .map_put(atoms::errstr().to_term(env), errstr)?;
    make_error_tuple3(env, reason, detail)
}

#[cfg(windows)]
// Helper: Create error tuple with WinAPI error details
fn make_winapi_error_tuple<'a>(
    env: Env<'a>,
    reason: &'static str,
    errnum: u32,
) -> NifResult<Term<'a>> {
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::winbase::{
        FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
        FORMAT_MESSAGE_IGNORE_INSERTS,
    };
    use winapi::um::winnt::MAKELANGID;

    let mut msg_buf: *mut u16 = std::ptr::null_mut();
    let flags =
        FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS;
    let lang = MAKELANGID(0, 0x2); // LANG_NEUTRAL, SUBLANG_DEFAULT

    let len = unsafe {
        FormatMessageW(
            flags,
            std::ptr::null(),
            errnum,
            lang,
            &mut msg_buf as *mut _ as *mut _,
            0,
            std::ptr::null_mut(),
        )
    };

    let errstr = if len == 0 {
        "Unknown WinAPI error".to_string()
    } else {
        let slice = unsafe { std::slice::from_raw_parts(msg_buf, len as usize) };
        match widestring::U16CStr::from_slice(slice) {
            Ok(wide) => wide.to_string_lossy(),
            Err(_) => "encoding_error".to_string(),
        }
    };

    unsafe {
        if !msg_buf.is_null() {
            winapi::um::heapapi::HeapFree(
                winapi::um::heapapi::GetProcessHeap(),
                0,
                msg_buf as *mut _,
            );
        }
    }

    let detail = rustler::types::map::map_new(env)
        // Use to_term() instead of term()
        .map_put(atoms::errno().to_term(env), errnum)?
        // Use to_term() instead of term()
        .map_put(atoms::errstr().to_term(env), errstr)?;
    make_error_tuple3(env, reason, detail)
}

// Helper: Convert Elixir term to a path
fn get_path_from_term<'a>(_env: Env<'a>, term: Term<'a>) -> NifResult<CString> {
    // Try binary first
    if let Ok(binary) = term.decode::<Binary>() {
        if binary.is_empty() {
            return Err(Error::Atom("invalid_path"));
        }
        // Validate UTF-8
        let slice = binary.as_slice();
        match std::str::from_utf8(slice) {
            Ok(_) => match CString::new(slice) {
                Ok(cstr) => Ok(cstr),
                Err(_) => Err(Error::Atom("invalid_path")),
            },
            Err(_) => Err(Error::Atom("invalid_path")),
        }
    } else if term.is_list() {
        // Fallback to string (list of chars)
        let path_str: String = match term.decode() {
            Ok(s) => s,
            Err(_) => return Err(Error::Atom("invalid_path")),
        };
        if path_str.is_empty() {
            return Err(Error::Atom("invalid_path"));
        }
        match CString::new(path_str) {
            Ok(cstr) => Ok(cstr),
            Err(_) => Err(Error::Atom("invalid_path")),
        }
    } else {
        Err(Error::Atom("invalid_path"))
    }
}

// Change the function signature to accept a single argument directly, instead of `args: &[Term<'a>]`
#[rustler::nif(schedule = "DirtyIo")]
fn stat_fs<'a>(env: Env<'a>, path_term: Term<'a>) -> NifResult<Term<'a>> {
    let path = match get_path_from_term(env, path_term) {
        Ok(path) => path,
        Err(_) => return make_error_tuple(env, "invalid_path"),
    };

    #[cfg(windows)]
    {
        // Convert to wide string for Windows API
        let wpath = match WideCString::from_str(path.to_str()?) {
            Ok(wp) => wp,
            Err(_) => return make_error_tuple(env, "path_conversion_failed"),
        };

        // Prepare long path with \\?\
        let long_wpath = if wpath.as_slice().starts_with(&[
            b'\\' as u16,
            b'\\' as u16,
            b'?' as u16,
            b'\\' as u16,
        ]) {
            wpath
        } else {
            match WideCString::from_str(format!("\\\\?\\{}", path.to_str()?)) {
                Ok(wp) => wp,
                Err(_) => return make_error_tuple(env, "path_conversion_failed"),
            }
        };

        // Check if path is a directory
        let attr = unsafe { GetFileAttributesW(long_wpath.as_ptr()) };
        if attr == INVALID_FILE_ATTRIBUTES {
            let err = unsafe { GetLastError() };
            return make_winapi_error_tuple(env, "not_directory", err);
        }
        if (attr & FILE_ATTRIBUTE_DIRECTORY) == 0 {
            return make_error_tuple(env, "not_directory");
        }

        // Get disk space
        let mut avail: u64 = 0;
        let mut total: u64 = 0;
        let mut free: u64 = 0;
        let success = unsafe {
            GetDiskFreeSpaceExW(
                long_wpath.as_ptr(),
                &mut avail as *mut _ as *mut _,
                &mut total as *mut _ as *mut _,
                &mut free as *mut _ as *mut _,
            )
        };
        if success == 0 {
            let err = unsafe { GetLastError() };
            return make_winapi_error_tuple(env, "winapi_failed", err);
        }

        let used = total.saturating_sub(free);
        let map = rustler::types::map::map_new(env)
            // Use to_term()
            .map_put(atoms::available().to_term(env), avail)?
            .map_put(atoms::free().to_term(env), free)?
            .map_put(atoms::total().to_term(env), total)?
            .map_put(atoms::used().to_term(env), used)?;
        // Use to_term()
        Ok(rustler::types::tuple::make_tuple(
            env,
            &[atoms::ok().to_term(env), map],
        ))
    }

    #[cfg(not(windows))]
    {
        // Check if path is a directory
        let os_path = Path::new(OsStr::from_bytes(path.as_bytes()));
        let metadata = match std::fs::metadata(&os_path) {
            Ok(m) => m,
            Err(e) => return make_errno_error_tuple(env, "not_directory", e),
        };
        if !metadata.is_dir() {
            return make_error_tuple(env, "not_directory");
        }

        #[cfg(target_os = "linux")]
        {
            let mut statfs_buf: libc::statfs64 = unsafe { mem::zeroed() };
            let path_cstr = path.as_ptr();
            let result = unsafe { statfs64(path_cstr, &mut statfs_buf) };
            if result != 0 {
                return make_errno_error_tuple(env, "statfs_failed", io::Error::last_os_error());
            }
            let avail = (statfs_buf.f_bavail as u64) * (statfs_buf.f_bsize as u64);
            let free = (statfs_buf.f_bfree as u64) * (statfs_buf.f_bsize as u64);
            let total = (statfs_buf.f_blocks as u64) * (statfs_buf.f_bsize as u64);
            let used = total.saturating_sub(free);
            let map = rustler::types::map::map_new(env)
                // Use to_term()
                .map_put(atoms::available().to_term(env), avail)?
                .map_put(atoms::free().to_term(env), free)?
                .map_put(atoms::total().to_term(env), total)?
                .map_put(atoms::used().to_term(env), used)?;
            // Use to_term()
            Ok(rustler::types::tuple::make_tuple(
                env,
                &[atoms::ok().to_term(env), map],
            ))
        }

        #[cfg(not(target_os = "linux"))]
        {
            let mut statvfs_buf: libc::statvfs64 = unsafe { mem::zeroed() };
            let path_cstr = path.as_ptr();
            let result = unsafe { statvfs64(path_cstr, &mut statvfs_buf) };
            if result != 0 {
                return make_errno_error_tuple(env, "statvfs_failed", io::Error::last_os_error());
            }
            let avail = (statvfs_buf.f_bavail as u64) * (statvfs_buf.f_frsize as u64);
            let free = (statvfs_buf.f_bfree as u64) * (statvfs_buf.f_frsize as u64);
            let total = (statvfs_buf.f_blocks as u64) * (statvfs_buf.f_frsize as u64);
            let used = total.saturating_sub(free);
            let map = rustler::types::map::map_new(env)
                // Use to_term()
                .map_put(atoms::available().to_term(env), avail)?
                .map_put(atoms::free().to_term(env), free)?
                .map_put(atoms::total().to_term(env), total)?
                .map_put(atoms::used().to_term(env), used)?;
            // Use to_term()
            Ok(rustler::types::tuple::make_tuple(
                env,
                &[atoms::ok().to_term(env), map],
            ))
        }
    }
}

rustler::init!("Elixir.DiskSpace");
