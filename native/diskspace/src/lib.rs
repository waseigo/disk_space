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
use std::ptr;
#[cfg(windows)]
use windows::core::PWSTR;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{GetLastError};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetFileAttributesW, FILE_ATTRIBUTE_DIRECTORY, INVALID_FILE_ATTRIBUTES};
#[cfg(windows)]
use windows::Win32::System::Memory::{GetProcessHeap, HeapFree, HEAP_FLAGS};
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::{FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
    FORMAT_MESSAGE_IGNORE_INSERTS};

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
// Helper: Create error tuple with WinAPI error details
fn make_winapi_error_tuple<'a>(env: Env<'a>, reason: Atom, errnum: u32) -> NifResult<Term<'a>> {
    let mut buffer_ptr: *mut u16 = ptr::null_mut();
    let flags =
        FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS;
    let lang: u32 = 0; // Use system default for better localization

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

    let errstr = if len == 0 {
        "Unknown WinAPI error".to_string()
    } else {
        let slice = unsafe { std::slice::from_raw_parts(buffer_ptr, len as usize + 1) };
        let wide_cstr = unsafe { widestring::U16CStr::from_slice_unchecked(slice) };
        wide_cstr.to_string_lossy().trim().to_string()
    };

    if !buffer_ptr.is_null() {
        if let Ok(heap) = unsafe { GetProcessHeap() } {
            unsafe { HeapFree(heap, HEAP_FLAGS(0), Some(buffer_ptr as *const _)); }
        }
    }

    let detail = rustler::types::map::map_new(env)
        .map_put(atoms::errno().to_term(env), errnum)?
        .map_put(atoms::errstr().to_term(env), errstr)?;
    make_error_tuple3(env, reason, detail)
}

// Helper: Convert Elixir term to a path
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
        let path_str = match path_cstr.to_str() {
            Ok(s) => s,
            Err(_) => return make_error_tuple(env, atoms::path_conversion_failed()),
        };

        let is_unc = path_str.starts_with("\\\\") && !path_str.starts_with("\\\\?\\");
        let long_path_str = if is_unc {
            format!("\\\\?\\UNC{}", &path_str[2..])
        } else if !path_str.starts_with("\\\\?\\") {
            format!("\\\\?\\{}", path_str)
        } else {
            path_str.to_string()
        };

        let wide_str = match widestring::WideCString::from_str(&long_path_str) {
            Ok(ws) => ws,
            Err(_) => return make_error_tuple(env, atoms::path_conversion_failed()),
        };
        let long_wpath = PCWSTR::from_raw(wide_str.as_ptr());

        let attr = unsafe { GetFileAttributesW(long_wpath) };
        if attr == INVALID_FILE_ATTRIBUTES {
            let err = unsafe { GetLastError() };
            return make_winapi_error_tuple(env, atoms::not_directory(), err.0);
        }
        if (attr & FILE_ATTRIBUTE_DIRECTORY.0) == 0 {
            return make_error_tuple(env, atoms::not_directory());
        }

        let mut avail: u64 = 0;
        let mut total: u64 = 0;
        let mut free: u64 = 0;
        let success = unsafe {
            GetDiskFreeSpaceExW(
                long_wpath,
                Some(&mut avail),
                Some(&mut total),
                Some(&mut free),
            )
        };
        if let Err(e) = success {
            return make_winapi_error_tuple(env, atoms::winapi_failed(), e.code().0 as u32);
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
                    let io_err = io::Error::from_raw_os_error(err as i32);
                    return make_errno_error_tuple(env, atoms::statfs_failed(), io_err);
                },
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
            let statvfs_buf: Statvfs = match statvfs(os_path) {
                Ok(buf) => buf,
                Err(err) => {
                    let io_err = io::Error::from_raw_os_error(err as i32);
                    return make_errno_error_tuple(env, atoms::statvfs_failed(), io_err);
                },
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