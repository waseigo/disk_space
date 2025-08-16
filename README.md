<img src="./etc/assets/disk_space_logo.png" width="100" height="100">

# DiskSpace

A small Elixir library with a NIF in Rust for getting disk usage statistics for a given filesystem path.

It returns information about total, used, free, and available disk space, by using native system calls. Optionally converts the results into human-readable strings with kibibytes etc. or kilobytes etc.

## Features

- Returns disk space metrics as a map with keys:
  - `:total` — total size of the filesystem
  - `:used` — bytes currently used
  - `:free` — bytes free on the filesystem
  - `:available` — bytes available to the current user (may be less than `:free` due to permissions)
- Provides both safe ([`stat/2`](https://hexdocs.pm/disk_space/DiskSpace.html#stat/2)) and bang ([`stat!/2`](https://hexdocs.pm/disk_space/DiskSpace.html#stat!/2)) functions, the latter raising [`DiskSpace.Error`](https://hexdocs.pm/disk_space/DiskSpace.Error.html) on errors
- Optional conversion of results from bytes into human-readable strings (in kB, KiB, etc.) with a keyword-list option that calls [`humanize/2`](https://hexdocs.pm/disk_space/DiskSpace.html#humanize/2)
- Supports Linux, macOS, Windows, NetBSD, FreeBSD, OpenBSD, DragonFlyBSD

## Installation

Add [`disk_space`](https://hex.pm/packages/disk_space) to your list of dependencies in `mix.exs`:

```elixir
def deps do
  [
    {:disk_space, "~> 1.0.0"}
  ]
end
```

## Usage examples

```elixir
iex(1)> DiskSpace.stat!("/tmp")
%{
  free: 39917449216,
  total: 225035927552,
  used: 185118478336,
  available: 28411740160
}
iex(2)> DiskSpace.stat("/tmp")
{:ok,
 %{
   free: 39917436928,
   total: 225035927552,
   used: 185118490624,
   available: 28411727872
 }}
iex(3)> DiskSpace.stat("/tmp", humanize: :binary)
{:ok,
 %{
   free: "37.18 GiB",
   total: "209.58 GiB",
   used: "172.41 GiB",
   available: "26.46 GiB"
 }}
iex(4)> DiskSpace.stat("/tmp", humanize: :decimal)
{:ok,
 %{
   free: "39.92 GB",
   total: "225.04 GB",
   used: "185.12 GB",
   available: "28.41 GB"
 }}
 iex(5)> DiskSpace.stat("/home/tisaak") |> DiskSpace.humanize()
{:ok,
 %{
   free: "37.18 GiB",
   total: "209.58 GiB",
   used: "172.41 GiB",
   available: "26.46 GiB"
 }}
iex(6)> DiskSpace.stat("/home/tisaak") |> DiskSpace.humanize(:decimal)
{:ok,
 %{
   free: "39.92 GB",
   total: "225.04 GB",
   used: "185.12 GB",
   available: "28.41 GB"
 }}
iex(7)> DiskSpace.stat("/yolo/swag")
{:error,
 %{
   info: %{errno: 2, errstr: "No such file or directory (os error 2)"},
   reason: :not_directory
 }}
iex(8)> DiskSpace.stat!("/yolo/swag")
** (DiskSpace.Error) DiskSpace error: %{info: %{errno: 2, errstr: "No such file or directory (os error 2)"}, reason: :not_directory}
    (disk_space 1.0.0) lib/disk_space.ex:84: DiskSpace.stat!/2
    iex:8: (file)
```

## Usage trick

In case you want to get results for a path that doesn't yet exist:

```elixir
  def recursively_check_disk_space(local_dir) when is_binary(local_dir) do
    local_dir |> Path.split() |> Enum.reduce_while(local_dir, fn _, acc ->
      case DiskSpace.stat(acc) do
        {:ok, info} -> {:halt, {:ok, info}}
        {:error, _} ->
          {:cont, acc |> Path.split() |> Enum.reverse() |> tl |> Enum.reverse() |> Path.join()}
      end
    end)
  end
```

This is pulled from my book [**Elixir File Browsing**](https://overbring.com/books/elixir-file-browsing/), in which the API client for the undocumented REST API of [File Browser](https://filebrowser.org) uses `DiskSpace` to check whether there is enough space on the target local path's mount point before downloading a resource from the server.

## Error handling

- [`stat/2`](https://hexdocs.pm/disk_space/DiskSpace.html#stat/2) returns `{:ok, stats_map}` or `{:error, info}`, where `info` is a map with populated `:reason` (atom) and `:info` (map or `nil`) with more information, if provided by the NIF.
- [`stat!/2`](https://hexdocs.pm/disk_space/DiskSpace.html#stat!/2) returns `stats_map` or raises [`DiskSpace.Error`](https://hexdocs.pm/disk_space/DiskSpace.Error.html) with the `{:error, info}` of [`stat/2`](https://hexdocs.pm/disk_space/DiskSpace.html#stat/2) as the message.

## Supported Elixir and OTP versions

In short:

- Tested and confirmed working on Elixir 1.14 (OTP 25) to 1.18 (OTP 27)
- Tested and confirmed working on Linux, Windows and the BSDs (amd64)
- Tested and confirmed working on macOS (arm64)
- Reported as also working on Elixir 1.18.4 (OTP 28), at least on macOS/arm64

### Build & test matrix

| OS                                                                        | Arch. | Elixir | OTP | Builds and `mix test` passes?  |
| ------------------------------------------------------------------------- | ----- | ------ | --- | ------------------------------ |
| Linux (Ubuntu/Debian)                                                     | amd64 | 1.14   | 25  | ✅                             |
| Linux (Ubuntu/Debian)                                                     | amd64 | 1.15   | 26  | ✅                             |
| Linux (Ubuntu/Debian)                                                     | amd64 | 1.16   | 26  | ✅                             |
| Linux (Ubuntu/Debian)                                                     | amd64 | 1.17   | 27  | ✅                             |
| Linux (Ubuntu/Debian)                                                     | amd64 | 1.18   | 27  | ✅                             |
| Linux (Ubuntu/Debian)                                                     | amd64 | 1.18.4 | 28  | ❔ Not tested, but should work |
| macOS                                                                     | arm64 | 1.14   | 25  | ✅                             |
| macOS                                                                     | arm64 | 1.15   | 26  | ✅                             |
| macOS                                                                     | arm64 | 1.16   | 26  | ✅                             |
| macOS                                                                     | arm64 | 1.17   | 27  | ✅                             |
| macOS                                                                     | arm64 | 1.18   | 27  | ✅                             |
| macOS                                                                     | arm64 | 1.18.4 | 28  | ✅ reported as working         |
| Windows                                                                   | amd64 | 1.14   | 25  | ✅                             |
| Windows                                                                   | amd64 | 1.15   | 26  | ✅                             |
| Windows                                                                   | amd64 | 1.16   | 26  | ✅                             |
| Windows                                                                   | amd64 | 1.17   | 27  | ✅                             |
| Windows                                                                   | amd64 | 1.18   | 27  | ✅                             |
| Windows                                                                   | amd64 | 1.18.4 | 28  | ❔ Not tested, but should work |
| [NetBSD 10.1](https://www.netbsd.org/releases/formal-10/NetBSD-10.1.html) | amd64 | 1.17.2 | 27  | ✅                             |
| [FreeBSD 14.3](https://www.freebsd.org/releases/14.3R/announce/)          | amd64 | 1.17.3 | 26  | ✅                             |
| [OpenBSD 7.7](https://www.openbsd.org/77.html)                            | amd64 | 1.18.3 | 27  | ✅                             |
| [DragonFlyBSD 6.4.2](https://www.dragonflybsd.org/release64/)             | amd64 | 1.16.3 | 25  | ✅                             |

See also: [GitHub Actions](https://github.com/waseigo/disk_space/actions) for Linux, macOS, Windows.

## Build requirements

Generally: Erlang development headers (for `erl_nif` functions), Rust.

### Linux (amd64)

- `erlang-dev` or `erlang-erts-dev` (Erlang development headers)
- `libc` development headers (usually installed by default)
- `rustc`

Example on Debian and its derivatives:

```
sudo apt-get install elixir erlang-dev rustc
```

### macOS (arm64)

```
xcode-select --install
```

- `clang`
- Erlang installed via [Homebrew](https://brew.sh/) or other means

### NetBSD (amd64)

✅ Tested on [version 10.1](https://www.netbsd.org/releases/formal-10/NetBSD-10.1.html) with Elixir 1.17.2, OTP 27.

```
pkgin update
pkgin install erlang elixir rust
```

### FreeBSD (amd64)

✅ Tested on [version 14.3](https://www.freebsd.org/releases/14.3R/announce/) with Elixir 1.17.3, OTP 26.

```
pkg update
pkg install erlang elixir ca_root_nss rust
```

### DragonFlyBSD (amd64)

✅ Tested on [version 6.4.2](https://www.dragonflybsd.org/release64/) with Elixir 1.16.3, OTP 25.

```
pkg update
pkg install erlang elixir rust
```

### OpenBSD (amd64)

✅ Tested on [version 7.7](https://www.openbsd.org/77.html) with Elixir 1.18.3, OTP 27.

```
pkg_add erlang-27.3.3v0 elixir-1.18.3 rust
```

### Windows (amd64)

1. Install Erlang/OTP with development headers:

   - Download the official installer from https://www.erlang.org/downloads (choose the latest stable version).
   - During installation, ensure "Development and debugging tools" is selected (this includes headers like `erl_nif.h`).
   - Add the Erlang bin directory to your `PATH` (e.g., `C:\Program Files\erl-27.0\bin`).

2. Install Elixir:

   - Download the installer from https://elixir-lang.org/install.html#windows.
   - Follow the instructions to add Elixir to your `PATH`.

3. Install Visual Studio Build Tools (required for Rust's MSVC toolchain):

   - Download from https://visualstudio.microsoft.com/downloads/ (under "Tools for Visual Studio", select "Build Tools for Visual Studio").
   - Run the installer and select the "C++ build tools" workload (includes MSVC compiler and linker).
   - No full Visual Studio IDE is needed—just the build tools.

4. Install Rust:
   - Download `rustup-init.exe` from https://www.rust-lang.org/tools/install.
   - Run it and select the default options, which install the stable MSVC toolchain (`stable-x86_64-pc-windows-msvc`).
   - Add Rust to your `PATH` if prompted (`cargo` and `rustc` should be accessible from the command line).

## Alternatives

Add [`:os_mon`](https://erlang.org/documentation/doc-16-rc2/lib/os_mon-2.11/doc/html/os_mon_app.html) in `:extra_applications` in `mix.exs`, then use [`get_disk_info/1`](https://erlang.org/documentation/doc-16-rc2/lib/os_mon-2.11/doc/html/disksup.html#get_disk_info/1) of [`disksup`](https://erlang.org/documentation/doc-16-rc2/lib/os_mon-2.11/doc/html/disksup.html) service.

### Comparison to alternatives

| Criterion                            | `:disksup.get_disk_info/1`                                                         | `disk_space.stat/2` and `stat!/2`                                                                                                                                                                         |
| ------------------------------------ | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| What it is                           | Function of a supervised process (`:os_mon`'s `disksup`)                           | Function relying on a NIF                                                                                                                                                                                 |
| Runtime requirements                 | `:os_mon` in `:extra_applications` in `mix.exs`                                    | None                                                                                                                                                                                                      |
| Compile-time requirements            | No, part of Erlang/OTP                                                             | Yes (Rust)                                                                                                                                                                                                |
| Returns                              | Total space, available space, and capacity (% of disk space used)                  | Returns total, used, free, available space                                                                                                                                                                |
| Return value type                    | 4-element tuple in list; first element: path as charlist; other elements: integers | 2-element tagged tuple; first element: `:ok` or `:error`; second element: map with atom keys and integer (bytes) or string (kB, KiB, etc.) values if `:ok`, map with `:reason` and OS `:info` if `:error` |
| Return units                         | kibibytes, percentage (as integers)                                                | bytes (integers) or human-readable strings                                                                                                                                                                |
| Optional conversion to KiB, kB, etc. | No                                                                                 | Yes (through `humanize/2`)                                                                                                                                                                                |
| Works with UNCs on Windows?          | Probably not (_"On WIN32 - All logical drives of type "FIXED_DISK" are checked."_) | Should work (not tested / cannot test)                                                                                                                                                                    |
| Well tested?                         | Yes                                                                                | Yes, according to [GitHub Actions](https://github.com/waseigo/disk_space/actions)                                                                                                                         |

## Use of GenAI

The following files were incrementally generated/adapted by xAI's Grok 4 model over multiple rounds of prompting for reviews and improvements that were suggested by Grok 4, GPT-5 and Gemini 2.5 Pro, and according to the warnings/errors of the GitHub Actions workflow across Linux, macOS, and Windows:

- [`lib.rs`](https://github.com/waseigo/disk_space/blob/main/native/diskspace/src/lib.rs)
- [`Cargo.toml`](https://github.com/waseigo/disk_space/blob/main/native/diskspace/Cargo.toml)
- [`build.yml`](https://github.com/waseigo/disk_space/blob/main/.github/workflows/build.yml)

## License

Apache-2.0

## Documentation

For more details, see the documentation at https://hexdocs.pm/disk_space.

## Related

- [DiskSpace - retrieve disk usage statistics for a given filesystem path](https://elixirforum.com/t/diskspace-retrieve-disk-usage-statistics-for-a-given-filesystem-path/72064) -- thread on ElixirForum
- [I let LLMs write an Elixir NIF in C; it mostly worked](https://overbring.com/blog/2025-08-13-writing-an-elixir-nif-with-genai/) -- blog post related to versions up to 0.4.0 that relied on C code generated by Grok 4 with code reviews by GPT-5 and Gemini 2.5 Flash
- [Comments on HackerNews](https://news.ycombinator.com/item?id=44914040) that spurred me to re-vibe-code it in Rust with Grok 4, as running [`splint`](https://splint.org) on the original C code revealed memory-safety issues
