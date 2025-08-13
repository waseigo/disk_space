# DiskSpace

A small Elixir library that provides functions to retrieve disk usage statistics for a given filesystem path.

It returns information about total, used, free, and available disk space, using native system calls for accuracy and performance.

Optionally converts the results into human-readable strings with kibibytes etc. or kilobytes etc.

## Features

- Returns disk space metrics as a map with keys:
  - `:total` — total size of the filesystem
  - `:used` — bytes currently used
  - `:free` — bytes free on the filesystem
  - `:available` — bytes available to the current user (may be less than `:free` due to permissions)
- Optional conversion of results from bytes into human-readable strings
- Supports Linux, macOS, BSD, and Windows (via native NIFs)
- Provides both safe (`stat/2`) and bang (`stat!/2`) variants (with `opts` keyword-list options for human-readable output), the latter raising on errors

## Installation

Add [`disk_space`](https://hex.pm/packages/disk_space) to your list of dependencies in `mix.exs`:

```elixir
def deps do
  [
    {:disk_space, "~> 0.2.0"}
  ]
end
```

## Build requirements

Since DiskSpace includes native code, you need the following build tools and development headers depending on your operating system:

### Linux

- `build-essential` (for `gcc`, `make`, etc.)
- `erlang-dev` or `erlang-erts-dev` (Erlang development headers)
- `libc` development headers (usually installed by default)

Example on Debian and its derivatives:

> `sudo apt-get install build-essential erlang-dev`

### macOS

- Xcode Command Line Tools (for gcc and make)
- Erlang installed via [Homebrew](https://brew.sh/) or other means

> `xcode-select --install`

### FreeBSD, NetBSD, DragonflyBSD, OpenBSD

- Appropriate compiler and make tools (usually available by default)
- Erlang development headers (install via `ports` or pkg manager)

### Windows

- Visual Studio with C++ build tools
- Erlang installed with development headers
- `nmake` or `mingw32-make` as your make tool

## Usage example

```elixir
iex(1)> DiskSpace.stat!("/tmp")
%{
  available: 32389640192,
  free: 43895349248,
  total: 225035927552,
  used: 181140578304
}
iex(2)> DiskSpace.stat("/tmp")
{:ok,
 %{
   available: 32389599232,
   free: 43895308288,
   total: 225035927552,
   used: 181140619264
 }}
iex(3)> DiskSpace.stat("/tmp", humanize: true)
{:ok,
 %{
   available: "30.17 GiB",
   free: "40.88 GiB",
   total: "209.58 GiB",
   used: "168.70 GiB"
 }}
iex(4)> DiskSpace.stat("/tmp", humanize: true, base: :decimal)
{:ok,
 %{
   available: "32.39 GB",
   free: "43.90 GB",
   total: "225.04 GB",
   used: "181.14 GB"
 }}
iex(5)> DiskSpace.stat("/home/tisaak") |> DiskSpace.humanize(:decimal)
{:ok,
 %{
   free: "43.86 GB",
   total: "225.04 GB",
   used: "181.18 GB",
   available: "32.35 GB"
 }}
iex(6)> DiskSpace.stat("/yolo/swag")
{:error,
 %{
   info: %{errno: 2, errstr: ~c"No such file or directory"},
   reason: :not_directory
 }}
iex(7)> DiskSpace.stat!("/yolo/swag")
** (DiskSpace.Error) DiskSpace error: {:error, %{info: nil, reason: %{info: %{errno: 2, errstr: ~c"No such file or directory"}, reason: :not_directory}}}
    (disk_space 0.1.1) lib/disk_space.ex:85: DiskSpace.stat!/2
    iex:7: (file)
```

## Error-handling

- `stat/2` returns `{:ok, stats_map}` or `{:error, info}`, where `info` is a map with populated `:reason` (atom) and `:info` (map or `nil`) with more information, if provided by the NIF.
- `stat!/2` returns `stats_map` or raises `DiskSpace.Error` with `stat/2`'s `{:error, info}` as the message.

## Alternatives

Add [`:os_mon`](https://erlang.org/documentation/doc-16-rc2/lib/os_mon-2.11/doc/html/os_mon_app.html) in `:extra_applications` in `mix.exs`, then use [get_disk_info/1](https://erlang.org/documentation/doc-16-rc2/lib/os_mon-2.11/doc/html/disksup.html#get_disk_info/1) of [`disksup`](https://erlang.org/documentation/doc-16-rc2/lib/os_mon-2.11/doc/html/disksup.html) service.

## Comparison to using `:os_mon`/`disksup`

| Criterion                            | `:disksup.get_disk_info/1`                                                         | `disk_space.stat/2` and `stat!/2`                                                                                                                                                                                                                                                   |
| ------------------------------------ | ---------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| What it is                           | Function of a supervised process (`:os_mon`'s `disksup`)                           | Function relying on a NIF                                                                                                                                                                                                                                                           |
| Runtime requirements                 | `:os_mon` in `:extra_applications` in `mix.exs`                                    | Nothing                                                                                                                                                                                                                                                                             |
| Need to compile?                     | No, part of Erlang                                                                 | Yes                                                                                                                                                                                                                                                                                 |
| Returns                              | Returns total space, available space, and capacity (% of disk space used)          | Returns total space, used space, free space, available to the current user                                                                                                                                                                                                          |
| Return value type                    | 4-element tuple in list; first element: path as charlist; other elements: integers | `{:ok, stats_map}` or `{:error, reason}` (`stat/2`), `stats_map` or raises exception (`stat!/2`), where `stats_map` is a plain Elixir map with atom keys with: integer values (bytes) if option `:humanize` is false (default),string values (KiB, kB, etc.) if `:humanize` is true |
| Return units                         | kibibytes, percentage (as integers)                                                | bytes (integers) or human-readable strings with `humanize: true`, either as KiB etc. (`base: :binary`, default) or kB etc. (`base: :decimal`)                                                                                                                                       |
| Interval                             | 30 minutes (default), configurable with `disk_space_check_interval`                | N/A - checks upon invocation                                                                                                                                                                                                                                                        |
| Optional conversion to KiB, kB, etc. | No                                                                                 | Yes, with `DiskSpace.humanize/2`                                                                                                                                                                                                                                                    |
| Works with UNCs on Windows?          | Probably not (_"On WIN32 - All logical drives of type "FIXED_DISK" are checked."_) | Should work (not tested / cannot test)                                                                                                                                                                                                                                              |
| Can alert you?                       | Yes, with `:alarm_handler`, signal `:disk_almost_full`, via `:os_mon`              | No, use it for "spot checks"                                                                                                                                                                                                                                                        |
| Well tested?                         | Yes                                                                                | No (not yet)                                                                                                                                                                                                                                                                        |

## Use of GenAI

`c_src/disk_space.c` was incrementally generated/adapted by xAI's Grok 3 model over multiple rounds of prompting for reviews and improvements that were suggested by Grok 3, GPT-5 and Gemini 2.5 Flash.

## Caveats

- Only tested on Debian Linux 12.
- _Should_ work on all POSIX systems, including the BSDs, but haven't tested it.
- _Should_ work on macOS, but I have no Apple PCs anymore to test it out.
- _Should_ work on Windows, but I have no Windows machines to test it with.
- The tests do not cover multiplatform compatibility.

Improvements, fixes and suggestions about potential issues welcome. Kindly submit a PR.

## License

Apache-2.0

## Documentation

For more details, see the documentation at https://hexdocs.pm/disk_space.

## Discussion thread

[ElixirForum: DiskSpace - retrieve disk usage statistics for a given filesystem path](https://elixirforum.com/t/diskspace-retrieve-disk-usage-statistics-for-a-given-filesystem-path/72064)
