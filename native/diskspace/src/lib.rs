use rustler::{Atom, Binary, Env, Error, NifResult, Term};
use std::ffi::CString;
#[cfg(unix)]
use std::io;
// Unix-specific imports
#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::path::Path;

// Windows-specific imports
#[cfg(windows)]
use windows::core::{PCWSTR, PWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::{GetLastError, LocalFree, HLOCAL};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetFileAttributesW, FILE_ATTRIBUTE_DIRECTORY, INVALID_FILE_ATTRIBUTES,
};
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::{
    FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
    FORMAT_MESSAGE_IGNORE_INSERTS,
};

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
// Helper: Create error tuple with WinAPI error details
fn make_winapi_error_tuple<'a>(env: Env<'a>, reason: Atom, errnum: u32) -> NifResult<Term<'a>> {
    // Ask the system for a human-readable message for `errnum`.
    let mut msg: PWSTR = PWSTR::null();
    let flags =
        FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS;

    // Safety: With ALLOCATE_BUFFER, FormatMessageW allocates and writes a pointer to `msg`.
    let len =
        unsafe { FormatMessageW(flags, None, errnum, 0, PWSTR(&mut msg.0 as *mut _), 0, None) };

    let errstr = if len == 0 {
        "Unknown WinAPI error".to_string()
    } else {
        // Convert the UTF-16 slice (exact length returned) to String and trim common CRLF.
        let slice = unsafe { std::slice::from_raw_parts(msg.0, len as usize) };
        widestring::U16Str::from_slice(slice)
            .to_string_lossy()
            .trim_end()
            .to_string()
    };

    if !msg.is_null() {
        // Safety: The buffer is owned by the system due to ALLOCATE_BUFFER and must be freed with LocalFree.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(msg.0 as *mut core::ffi::c_void)));
        }
    }

    let detail = rustler::types::map::map_new(env)
        .map_put(atoms::errno().to_term(env), errnum)?
        .map_put(atoms::errstr().to_term(env), errstr)?;
    make_error_tuple3(env, reason, detail)
}

// Helper: Convert Elixir term to a path (as bytes). For Windows we later turn UTF-8 -> UTF-16.
fn get_path_from_term<'a>(_env: Env<'a>, term: Term<'a>) -> NifResult<CString> {
    // Try binary first
    let binary = match term.decode::<Binary>() {
        Ok(b) => b,
        Err(_) => {
            // Fallback to string (list of chars)
            let path_str: String = match term.decode() {
                Ok(s) => s,
                Err(_) => return Err(Error::BadArg),
            };
            match CString::new(path_str) {
                Ok(cstr) => return Ok(cstr),
                Err(_) => return Err(Error::BadArg),
            }
        }
    };

    if binary.is_empty() {
        return Err(Error::BadArg);
    }

    match CString::new(binary.as_slice()) {
        Ok(cstr) => Ok(cstr),
        Err(_) => Err(Error::BadArg),
    }
}

#[rustler::nif(schedule = "DirtyIo")]
fn stat_fs<'a>(env: Env<'a>, path_term: Term<'a>) -> NifResult<Term<'a>> {
    let path_cstr = match get_path_from_term(env, path_term) {
        Ok(path) => path,
        Err(_) => return make_error_tuple(env, atoms::invalid_path()),
    };

    #[cfg(windows)]
    {
        // The NIF expects UTF-8 input; Windows APIs require UTF-16.
        let path_str = match path_cstr.to_str() {
            Ok(s) => s,
            Err(_) => return make_error_tuple(env, atoms::path_conversion_failed()),
        };

        // Long-path handling. Preserve existing \?\ prefix; convert UNC to \\?\UNC\...
        let is_unc = path_str.starts_with("\\\\") && !path_str.starts_with("\\\\?\\");
        let mut long_path_str = if is_unc {
            format!("\\\\?\\UNC{}", &path_str[2..])
        } else if !path_str.starts_with("\\\\?\\") {
            format!("\\\\?\\{}", path_str)
        } else {
            path_str.to_string()
        };

        // Normalize bare drive roots like "\\?\C:" -> "\\?\C:\" for more predictable API behavior.
        if long_path_str.len() == 6 && long_path_str.ends_with(':') {
            long_path_str.push('\\');
        }

        let wide_str = match widestring::WideCString::from_str(&long_path_str) {
            Ok(ws) => ws,
            Err(_) => return make_error_tuple(env, atoms::path_conversion_failed()),
        };
        let wpath = PCWSTR(wide_str.as_ptr());

        let attr = unsafe { GetFileAttributesW(wpath) };
        if attr == INVALID_FILE_ATTRIBUTES {
            let err = unsafe { GetLastError() };
            return make_winapi_error_tuple(env, atoms::winapi_failed(), err.0);
        }
        if (attr & FILE_ATTRIBUTE_DIRECTORY.0) == 0 {
            return make_error_tuple(env, atoms::not_directory());
        }

        let mut avail: u64 = 0;
        let mut total: u64 = 0;
        let mut free: u64 = 0;

        // Safety: pointers to out-params are valid for the duration of the call.
        let ok = unsafe {
            GetDiskFreeSpaceExW(wpath, Some(&mut avail), Some(&mut total), Some(&mut free))
        };
        if !ok.as_bool() {
            let err = unsafe { GetLastError() };
            return make_winapi_error_tuple(env, atoms::winapi_failed(), err.0);
        }

        let used = total.saturating_sub(free);
        let map = rustler::types::map::map_new(env)
            .map_put(atoms::available().to_term(env), avail)?
            .map_put(atoms::free().to_term(env), free)?
            .map_put(atoms::total().to_term(env), total)?
            .map_put(atoms::used().to_term(env), used)?;
        return Ok(rustler::types::tuple::make_tuple(
            env,
            &[atoms::ok().to_term(env), map],
        ));
    }

    #[cfg(unix)]
    {
        let os_path = Path::new(OsStr::from_bytes(path_cstr.as_bytes()));
        let metadata = match std::fs::metadata(&os_path) {
            Ok(m) => m,
            Err(e) => return make_errno_error_tuple(env, atoms::not_directory(), e),
        };
        if !metadata.is_dir() {
            return make_error_tuple(env, atoms::not_directory());
        }

        #[cfg(target_os = "linux")]
        {
            let statfs_buf: Statfs = match statfs(os_path) {
                Ok(buf) => buf,
                Err(err) => {
                    let io_err = io::Error::from_raw_os_error(err as i32); // nix Errno layout matches C errno
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
            return Ok(rustler::types::tuple::make_tuple(
                env,
                &[atoms::ok().to_term(env), map],
            ));
        }

        #[cfg(not(target_os = "linux"))]
        {
            let statvfs_buf: Statvfs = match statvfs(os_path) {
                Ok(buf) => buf,
                Err(err) => {
                    let io_err = io::Error::from_raw_os_error(err as i32); // nix Errno layout matches C errno
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
            return Ok(rustler::types::tuple::make_tuple(
                env,
                &[atoms::ok().to_term(env), map],
            ));
        }
    }
}

rustler::init!("Elixir.DiskSpace");
