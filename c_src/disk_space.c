// SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
// SPDX-License-Identifier: Apache-2.0
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
#include <sys/statfs.h>  // For statfs fallback on Linux
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

// Helper: Create error tuple with errno details for POSIX errors
static ERL_NIF_TERM make_errno_error_tuple(ErlNifEnv* env, ERL_NIF_TERM reason, int errnum) {
	// Get required buffer size for error message
	char buf[256]; // Initial buffer for most cases
	char *msg;
	size_t buf_size = 256;

#if defined(__GLIBC__) && defined(_GNU_SOURCE)
	// GNU-specific strerror_r returns char*
	msg = strerror_r(errnum, buf, buf_size);
	if (msg != buf) {
		// If msg points to an internal static buffer, copy it
		size_t msg_len = strlen(msg) + 1;
		char *new_buf = enif_alloc(msg_len);
		if (!new_buf) {
			return make_error_tuple(env, atom_alloc_failed);
		}
		strcpy(new_buf, msg);
		msg = new_buf;
	}
#else
	// XSI-compliant strerror_r returns int
	if (strerror_r(errnum, buf, buf_size) != 0) {
		// Buffer may be too small or error occurred; try dynamic allocation
		buf_size = 1024; // Larger size for edge cases
		char *new_buf = enif_alloc(buf_size);
		if (!new_buf) {
			return make_error_tuple(env, atom_alloc_failed);
		}
		if (strerror_r(errnum, new_buf, buf_size) == 0) {
			msg = new_buf;
		} else {
			enif_free(new_buf);
			msg = "Unknown error";
		}
	} else {
		msg = buf;
	}
#endif
	ERL_NIF_TERM errnum_term = enif_make_int(env, errnum);
	ERL_NIF_TERM errstr_term = enif_make_string(env, msg, ERL_NIF_UTF8);
	ERL_NIF_TERM detail;
	ERL_NIF_TERM keys[] = { atom_errno, atom_errstr };
	ERL_NIF_TERM vals[] = { errnum_term, errstr_term };
	if (!enif_make_map_from_arrays(env, keys, vals, 2, &detail)) {
		if (msg != buf) enif_free(msg);
		return make_error_tuple(env, atom_alloc_failed);
	}
	if (msg != buf) enif_free(msg);
	return make_error_tuple3(env, reason, detail);
}

// Helper: Validate UTF-8 encoding in a binary
static int is_valid_utf8(const unsigned char *data, size_t size) {
	size_t i = 0;
	while (i < size) {
		if (data[i] < 0x80) {
			// 1-byte sequence (ASCII)
			i++;
		} else if ((data[i] & 0xE0) == 0xC0) {
			// 2-byte sequence
			if (i + 1 >= size || (data[i + 1] & 0xC0) != 0x80) return 0;
			if ((data[i] & 0x1E) == 0) return 0; // Overlong encoding
			i += 2;
		} else if ((data[i] & 0xF0) == 0xE0) {
			// 3-byte sequence
			if (i + 2 >= size || (data[i + 1] & 0xC0) != 0x80 || (data[i + 2] & 0xC0) != 0x80) return 0;
			if ((data[i] == 0xE0 && (data[i + 1] & 0xE0) == 0x80) || // Overlong
			        (data[i] == 0xED && (data[i + 1] & 0xE0) == 0xA0)) return 0; // Surrogates
			i += 3;
		} else if ((data[i] & 0xF8) == 0xF0) {
			// 4-byte sequence
			if (i + 3 >= size || (data[i + 1] & 0xC0) != 0x80 || (data[i + 2] & 0xC0) != 0x80 ||
			        (data[i + 3] & 0xC0) != 0x80) return 0;
			if ((data[i] == 0xF0 && (data[i + 1] & 0xF0) == 0x80) || // Overlong
			        (data[i] > 0xF4)) return 0; // Beyond Unicode range
			i += 4;
		} else {
			// Invalid start byte
			return 0;
		}
	}
	return 1;
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
		// Validate UTF-8
		if (!is_valid_utf8(bin.data, bin.size)) {
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

	// Fallback to string terms (list of chars)
	if (!enif_is_list(env, term)) {
		return NULL;
	}
	// Estimate initial buffer size
	char* path = enif_alloc(256);
	if (!path) {
		return NULL;
	}
	int len = enif_get_string(env, term, path, 256, ERL_NIF_UTF8);
	if (len < 0) { // Invalid string or not UTF-8
		enif_free(path);
		return NULL;
	} else if (len > 255) { // Buffer too small
		enif_free(path);
		path = enif_alloc(len);
		if (!path) {
			return NULL;
		}
		if (enif_get_string(env, term, path, len, ERL_NIF_UTF8) <= 0) {
			enif_free(path);
			return NULL;
		}
	} else if (len <= 1) { // Empty string or error
		enif_free(path);
		return NULL;
	}
	return path;
}

#ifdef _WIN32
// Convert Windows API error code to UTF-8 string term.
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
	ERL_NIF_TERM errnum_term = enif_make_uint(env, errnum);
	ERL_NIF_TERM detail;
	ERL_NIF_TERM keys[] = { atom_errno, atom_errstr };
	ERL_NIF_TERM vals[] = { errnum_term, errstr_term };
	if (!enif_make_map_from_arrays(env, keys, vals, 2, &detail)) {
		return make_error_tuple(env, atom_alloc_failed);
	}
	return make_error_tuple3(env, reason, detail);
}

// Helper: Prepare path with \\?\ prefix for long path support
static wchar_t* prepare_long_path(ErlNifEnv* env, const wchar_t* wpath, int* wpath_len) {
	const wchar_t* prefix = L"\\\\?\\";
	int prefix_len = wcslen(prefix);
	if (wcsncmp(wpath, prefix, prefix_len) != 0) {
		wchar_t* long_path = enif_alloc(sizeof(wchar_t) * (*wpath_len + prefix_len + 1));
		if (!long_path) {
			return NULL;
		}
		wcscpy(long_path, prefix);
		wcscat(long_path, wpath);
		*wpath_len += prefix_len;
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
//   where values are in bytes.
// - Output on error: {:error, reason, %{errno: integer, errstr: string}} or {:error, reason}
// - Platform behavior:
//   - POSIX: Path must be a directory; follows symlinks to check the target.
//     Uses statvfs, with statfs fallback on Linux.
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
	// Convert UTF-8 C string to UTF-16LE (wchar_t) for Windows API
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
	if (!long_wpath) {
		enif_free(path);
		enif_free(wpath);
		return make_error_tuple(env, atom_alloc_failed);
	}
	enif_free(wpath); // No longer needed

	// Check if the path exists and is a directory
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
		uint64_t used = 0;
		if (total.QuadPart >= free.QuadPart) {
			used = total.QuadPart - free.QuadPart;
		}

		ERL_NIF_TERM keys[] = {
			atom_available,
			atom_free,
			atom_total,
			atom_used
		};
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
	uint64_t avail = 0, free = 0, total = 0;

	if (statvfs_ret != 0) {
#ifdef __linux__
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
	} else {
		avail = (uint64_t)statvfs_buf.f_bavail * (uint64_t)statvfs_buf.f_frsize;
		free  = (uint64_t)statvfs_buf.f_bfree  * (uint64_t)statvfs_buf.f_frsize;
		total = (uint64_t)statvfs_buf.f_blocks * (uint64_t)statvfs_buf.f_frsize;
	}

	uint64_t used = 0;
	if (total >= free) {
		used = total - free;
	}

	ERL_NIF_TERM keys[] = {
		atom_available,
		atom_free,
		atom_total,
		atom_used
	};
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