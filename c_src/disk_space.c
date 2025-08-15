// This file includes code generated or modified by xAI's Grok 3 
// over multiple rounds of prompting for reviews and improvements that were
// suggested by Grok 3, GPT-5 and Gemini 2.5 Flash.

#include "erl_nif.h"
#include <errno.h>
#include <string.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <sys/statvfs.h>
#include <sys/stat.h>
#include <limits.h>
#include <unistd.h>
#ifdef __linux__
#include <sys/statfs.h> // For statfs fallback on Linux
#endif
#endif

// Atoms for consistent error and status reporting
static ERL_NIF_TERM atom_error;
static ERL_NIF_TERM atom_ok;
static ERL_NIF_TERM atom_wrong_arity;
static ERL_NIF_TERM atom_invalid_path;
static ERL_NIF_TERM atom_alloc_failed;
static ERL_NIF_TERM atom_path_conversion_failed;
static ERL_NIF_TERM atom_not_directory;
static ERL_NIF_TERM atom_winapi_failed;
static ERL_NIF_TERM atom_statvfs_failed;
static ERL_NIF_TERM atom_statfs_failed;
static ERL_NIF_TERM atom_available;
static ERL_NIF_TERM atom_free;
static ERL_NIF_TERM atom_total;
static ERL_NIF_TERM atom_used;
static ERL_NIF_TERM atom_errno;
static ERL_NIF_TERM atom_errstr;

// Helper: Create {error, Reason} tuple
static ERL_NIF_TERM make_error_tuple(ErlNifEnv* env, ERL_NIF_TERM reason) {
    return enif_make_tuple2(env, atom_error, reason);
}

// Helper: Create {error, Reason, Detail} tuple
static ERL_NIF_TERM make_error_tuple3(ErlNifEnv* env, ERL_NIF_TERM reason, ERL_NIF_TERM detail) {
    return enif_make_tuple3(env, atom_error, reason, detail);
}

// Helper: Create error detail map {errno: integer, errstr: string}
static ERL_NIF_TERM make_error_detail(ErlNifEnv* env, int errnum, const char* errstr) {
    ERL_NIF_TERM detail;
    ERL_NIF_TERM keys[] = { atom_errno, atom_errstr };
    ERL_NIF_TERM vals[] = { enif_make_int(env, errnum), enif_make_string(env, errstr, ERL_NIF_UTF8) };
    if (!enif_make_map_from_arrays(env, keys, vals, 2, &detail)) {
        return 0; // Caller must handle failure
    }
    return detail;
}

// Helper: Create error tuple with errno details for POSIX errors
static ERL_NIF_TERM make_errno_error_tuple(ErlNifEnv* env, ERL_NIF_TERM reason, int errnum) {
    char buf[1024]; // Sufficient for most error messages
    char* msg;
#if defined(__GLIBC__) && defined(_GNU_SOURCE)
    // GNU-specific strerror_r returns char*
    msg = strerror_r(errnum, buf, sizeof(buf));
#else
    // XSI-compliant strerror_r returns int
    if (strerror_r(errnum, buf, sizeof(buf)) != 0) {
        msg = "Unknown error";
    } else {
        msg = buf;
    }
#endif
    ERL_NIF_TERM detail = make_error_detail(env, errnum, msg);
    if (!detail) {
        return make_error_tuple(env, atom_alloc_failed);
    }
    return make_error_tuple3(env, reason, detail);
}

// Convert Elixir term to a null-terminated UTF-8 C string.
// Returns NULL if conversion fails, input is empty, or not valid UTF-8.
// The caller is responsible for freeing the returned memory using enif_free().
static char* get_path_from_term(ErlNifEnv* env, ERL_NIF_TERM term) {
    ErlNifBinary bin;
    if (enif_inspect_binary(env, term, &bin)) {
        if (bin.size == 0) {
            return NULL;
        }
        char* path = enif_alloc(bin.size + 1);
        if (!path) {
            return NULL;
        }
        memcpy(path, bin.data, bin.size);
        path[bin.size] = '\0'; // Ensure null termination
        return path;
    }
    if (!enif_is_list(env, term)) {
        return NULL;
    }
    // Get required string length
    int len = enif_get_string(env, term, NULL, 0, ERL_NIF_UTF8);
    if (len <= 0) {
        return NULL; // Empty, invalid, or not UTF-8
    }
    char* path = enif_alloc((size_t)len);
    if (!path) {
        return NULL;
    }
    if (enif_get_string(env, term, path, len, ERL_NIF_UTF8) <= 0) {
        enif_free(path);
        return NULL;
    }
    return path;
}

#ifdef _WIN32
// Convert Windows API error code to UTF-8 string term
static ERL_NIF_TERM winapi_error_to_term(ErlNifEnv* env, DWORD err) {
    LPVOID msg_buf = NULL;
    DWORD flags = FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS;
    DWORD lang = MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT);
    if (FormatMessageW(flags, NULL, err, lang, (LPWSTR)&msg_buf, 0, NULL) == 0) {
        return enif_make_string(env, "Unknown WinAPI error", ERL_NIF_UTF8);
    }
    int utf8_len = WideCharToMultiByte(CP_UTF8, 0, (LPCWSTR)msg_buf, -1, NULL, 0, NULL, NULL);
    if (utf8_len == 0) {
        LocalFree(msg_buf);
        return enif_make_string(env, "encoding_error", ERL_NIF_UTF8);
    }
    char* utf8_str = enif_alloc(utf8_len);
    if (!utf8_str) {
        LocalFree(msg_buf);
        return enif_make_string(env, "alloc_failed", ERL_NIF_UTF8);
    }
    if (WideCharToMultiByte(CP_UTF8, 0, (LPCWSTR)msg_buf, -1, utf8_str, utf8_len, NULL, NULL) == 0) {
        enif_free(utf8_str);
        LocalFree(msg_buf);
        return enif_make_string(env, "encoding_failed_write", ERL_NIF_UTF8);
    }
    ERL_NIF_TERM ret = enif_make_string(env, utf8_str, ERL_NIF_UTF8);
    enif_free(utf8_str);
    LocalFree(msg_buf);
    return ret;
}

// Helper: Create error tuple with WinAPI error details
static ERL_NIF_TERM make_winapi_error_tuple(ErlNifEnv* env, ERL_NIF_TERM reason, DWORD errnum) {
    ERL_NIF_TERM errstr_term = winapi_error_to_term(env, errnum);
    ERL_NIF_TERM detail = make_error_detail(env, (int)errnum, enif_get_string_length(env, errstr_term, NULL, ERL_NIF_UTF8) > 0 ? NULL : "Unknown WinAPI error");
    if (!detail) {
        return make_error_tuple(env, atom_alloc_failed);
    }
    return make_error_tuple3(env, reason, detail);
}

// Helper: Prepare path with \\?\ prefix for long path support
static wchar_t* prepare_long_path(ErlNifEnv* env, const wchar_t* wpath, int* wpath_len) {
    const wchar_t* prefix = L"\\\\?\\";
    size_t prefix_len = wcslen(prefix);
    if (wcsncmp(wpath, prefix, prefix_len) != 0) {
        wchar_t* long_path = enif_alloc(sizeof(wchar_t) * (*wpath_len + prefix_len + 1));
        if (!long_path) {
            return NULL;
        }
        wcscpy(long_path, prefix);
        wcscat(long_path, wpath);
        *wpath_len += (int)prefix_len;
        return long_path;
    }
    wchar_t* path_copy = enif_alloc(sizeof(wchar_t) * (*wpath_len + 1));
    if (!path_copy) {
        return NULL;
    }
    wcscpy(path_copy, wpath);
    return path_copy;
}
#endif // _WIN32

// NIF function: stat_fs(Path :: binary | string) -> {ok, map} | {error, reason, detail}
// Returns filesystem statistics for a directory path.
// - Input: Path as a binary or string (UTF-8 encoded).
// - Output on success: {:ok, %{available: integer, free: integer, total: integer, used: integer}}
// - Output on error: {:error, reason, %{errno: integer, errstr: string}} or {:error, reason}
// - Platform behavior:
//   - POSIX: Path must be a directory; follows symlinks to check the target.
//     Uses statvfs, with statfs fallback on Linux and BSDs.
//   - Windows: Path must be a directory; reports volume statistics using GetDiskFreeSpaceExW.
//     Supports long paths with \\?\ prefix.
// - Runs on a dirty I/O scheduler due to potentially blocking system calls.
static ERL_NIF_TERM stat_fs(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]) {
    if (argc != 1) {
        return make_error_tuple(env, atom_wrong_arity);
    }
    char* path = get_path_from_term(env, argv[0]);
    if (!path) {
        return make_error_tuple(env, atom_invalid_path);
    }
#ifdef _WIN32
    // Convert UTF-8 to UTF-16LE for Windows API
    int wpath_len = MultiByteToWideChar(CP_UTF8, 0, path, -1, NULL, 0);
    if (wpath_len == 0) {
        enif_free(path);
        return make_error_tuple(env, atom_path_conversion_failed);
    }
    wchar_t* wpath = enif_alloc(sizeof(wchar_t) * wpath_len);
    if (!wpath) {
        enif_free(path);
        return make_error_tuple(env, atom_alloc_failed);
    }
    if (MultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, wpath_len) == 0) {
        enif_free(path);
        enif_free(wpath);
        return make_error_tuple(env, atom_path_conversion_failed);
    }
    // Prepare path with \\?\ for long path support
    wchar_t* long_wpath = prepare_long_path(env, wpath, &wpath_len);
    enif_free(wpath); // No longer needed
    if (!long_wpath) {
        enif_free(path);
        return make_error_tuple(env, atom_alloc_failed);
    }
    // Check if path exists and is a directory
    DWORD attr = GetFileAttributesW(long_wpath);
    if (attr == INVALID_FILE_ATTRIBUTES) {
        DWORD err = GetLastError();
        enif_free(path);
        enif_free(long_wpath);
        return make_winapi_error_tuple(env, atom_not_directory, err);
    }
    if (!(attr & FILE_ATTRIBUTE_DIRECTORY)) {
        enif_free(path);
        enif_free(long_wpath);
        return make_error_tuple(env, atom_not_directory);
    }
    ULARGE_INTEGER avail, total, free;
    if (GetDiskFreeSpaceExW(long_wpath, &avail, &total, &free)) {
        uint64_t used = total.QuadPart >= free.QuadPart ? total.QuadPart - free.QuadPart : 0;
        ERL_NIF_TERM keys[] = { atom_available, atom_free, atom_total, atom_used };
        ERL_NIF_TERM vals[] = {
            enif_make_uint64(env, avail.QuadPart),
            enif_make_uint64(env, free.QuadPart),
            enif_make_uint64(env, total.QuadPart),
            enif_make_uint64(env, used)
        };
        ERL_NIF_TERM map;
        if (!enif_make_map_from_arrays(env, keys, vals, 4, &map)) {
            enif_free(path);
            enif_free(long_wpath);
            return make_error_tuple(env, atom_alloc_failed);
        }
        enif_free(path);
        enif_free(long_wpath);
        return enif_make_tuple2(env, atom_ok, map);
    }
    DWORD err = GetLastError();
    enif_free(path);
    enif_free(long_wpath);
    return make_winapi_error_tuple(env, atom_winapi_failed, err);
#else // POSIX systems
    // Check path existence and directory status
    struct stat st;
    if (stat(path, &st) != 0) {
        int err = errno;
        enif_free(path);
        return make_errno_error_tuple(env, atom_not_directory, err);
    }
    if (!S_ISDIR(st.st_mode)) {
        enif_free(path);
        return make_error_tuple(env, atom_not_directory);
    }
    struct statvfs statvfs_buf;
    int statvfs_ret = statvfs(path, &statvfs_buf);
    uint64_t avail, free, total;
    if (statvfs_ret == 0) {
        avail = (uint64_t)statvfs_buf.f_bavail * (uint64_t)statvfs_buf.f_frsize;
        free = (uint64_t)statvfs_buf.f_bfree * (uint64_t)statvfs_buf.f_frsize;
        total = (uint64_t)statvfs_buf.f_blocks * (uint64_t)statvfs_buf.f_frsize;
    } else {
#if defined(__linux__) || defined(__FreeBSD__) || defined(__NetBSD__) || defined(__OpenBSD__)
        struct statfs statfs_buf;
        if (statfs(path, &statfs_buf) != 0) {
            int err = errno;
            enif_free(path);
            return make_errno_error_tuple(env, atom_statfs_failed, err);
        }
        avail = (uint64_t)statfs_buf.f_bavail * (uint64_t)statfs_buf.f_bsize;
        free = (uint64_t)statfs_buf.f_bfree * (uint64_t)statfs_buf.f_bsize;
        total = (uint64_t)statfs_buf.f_blocks * (uint64_t)statfs_buf.f_bsize;
#else
        int err = errno;
        enif_free(path);
        return make_errno_error_tuple(env, atom_statvfs_failed, err);
#endif
    }
    uint64_t used = total >= free ? total - free : 0;
    ERL_NIF_TERM keys[] = { atom_available, atom_free, atom_total, atom_used };
    ERL_NIF_TERM vals[] = {
        enif_make_uint64(env, avail),
        enif_make_uint64(env, free),
        enif_make_uint64(env, total),
        enif_make_uint64(env, used)
    };
    ERL_NIF_TERM map;
    if (!enif_make_map_from_arrays(env, keys, vals, 4, &map)) {
        enif_free(path);
        return make_error_tuple(env, atom_alloc_failed);
    }
    enif_free(path);
    return enif_make_tuple2(env, atom_ok, map);
#endif
}

// NIF load callback: initialize atoms
static int load(ErlNifEnv* env, void** priv_data, ERL_NIF_TERM load_info) {
    atom_error = enif_make_atom(env, "error");
    atom_ok = enif_make_atom(env, "ok");
    atom_wrong_arity = enif_make_atom(env, "wrong_arity");
    atom_invalid_path = enif_make_atom(env, "invalid_path");
    atom_alloc_failed = enif_make_atom(env, "alloc_failed");
    atom_path_conversion_failed = enif_make_atom(env, "path_conversion_failed");
    atom_not_directory = enif_make_atom(env, "not_directory");
    atom_winapi_failed = enif_make_atom(env, "winapi_failed");
    atom_statvfs_failed = enif_make_atom(env, "statvfs_failed");
    atom_statfs_failed = enif_make_atom(env, "statfs_failed");
    atom_available = enif_make_atom(env, "available");
    atom_free = enif_make_atom(env, "free");
    atom_total = enif_make_atom(env, "total");
    atom_used = enif_make_atom(env, "used");
    atom_errno = enif_make_atom(env, "errno");
    atom_errstr = enif_make_atom(env, "errstr");
    return 0;
}

// Exported NIF functions
static ErlNifFunc nif_funcs[] = {
    {"stat_fs", 1, stat_fs, ERL_NIF_DIRTY_JOB_IO_BOUND}
};

ERL_NIF_INIT(Elixir.DiskSpace, nif_funcs, load, NULL, NULL, NULL)