# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

defmodule DiskSpace do
  @moduledoc """
  Provides disk space information by interfacing with a native NIF library.

  The main function `stat/2` returns disk space stats for a given filesystem path.

  It also provides a bang variant `stat!/2`, which raises a `DiskSpace.Error` exception on error.

  Both functions support optionally humanizing the output into
  strings (`:humanize` and `:base` options).
  """
  @on_load :load_nifs

  # Compute extension at compile-time
  @lib_ext (case(:os.type()) do
              {:win32, _} -> ".dll"
              {:unix, :darwin} -> ".dylib"
              {:unix, _} -> ".so"
            end)

  defp load_nifs do
    priv_dir = :code.priv_dir(:disk_space) |> to_string()
    base_name = "disk_space"
    path = Path.join(priv_dir, base_name <> @lib_ext)
    :erlang.load_nif(to_charlist(path), 0)
  end

  defmodule Error do
    @moduledoc """
    Exception raised when a disk space operation fails.

    Contains a message describing the error reason.
    """
    defexception [:message]

    @impl true
    def exception(reason) do
      %__MODULE__{message: "DiskSpace error: #{inspect(reason)}"}
    end
  end

  # stub with minimal arity for NIF binding
  defp stat_fs(_path), do: :erlang.nif_error(:nif_not_loaded)

  @doc """
  Retrieves disk space statistics for the given `path`.

  Returns `{:ok, stats_map}` where `stats_map` is a plain Elixir map with the following keys and values in **bytes**:

    * `:available` - the number of bytes available to the current user.
      This excludes space reserved by the system or for privileged processes.

    * `:free` - the total number of free bytes on the filesystem,
      including space that may be reserved and unavailable to the current user.

    * `:total` - the total size of the filesystem in bytes.

    * `:used` - the number of bytes currently used (total - free).

    Returns `{:error, info}` if the operation fails, where `info` is a map with keys `:reason` and `:info`; `:reason` is always an atom, `:info` provides more information or is `nil`, depending on what is reported by the NIF.

  ## Options

    * `:humanize` (boolean) - whether to convert byte counts into human-readable strings.
      Defaults to `false`.

    * `:base` (`:binary` or `:decimal`) - base used for human-readable formatting.
      Defaults to `:binary` (kibibytes, etc.). `:decimal` for kilobytes etc.
  """

  # no point in a guard, as the stub function is replaced and
  # c_src/disk_space.c already checks the type of the path argument
  def stat(path, opts \\ []) when is_bitstring(path) and is_list(opts) do
    humanize? = Keyword.get(opts, :humanize, false)
    base = Keyword.get(opts, :base, :binary)

    path
    |> stat_fs()
    |> reshape_error_tuple()
    |> then(fn stats -> if humanize?, do: humanize(stats, base), else: stats end)
  end

  @doc """
  Same as `stat/2` (and with the same `opts` keyword-list options), but returns the `stats_map` plain Elixir map directly or raises `DiskSpace.Error` on failure.
  """
  def stat!(path, opts \\ []) do
    case stat(path, opts) do
      {:ok, stats} -> stats
      {:error, _info} = failure -> raise Error, reshape_error_tuple(failure)
    end
  end

  defp reshape_error_tuple({:error, reason}), do: {:error, %{reason: reason, info: nil}}
  defp reshape_error_tuple({:error, reason, info}), do: {:error, %{reason: reason, info: info}}
  defp reshape_error_tuple({:ok, stats_map} = success) when is_map(stats_map), do: success

  @doc """
  Converts disk space statistics coming from `stat/2` and `stat!/2` from raw byte counts to human-readable strings.

  Accepts either a tuple `{:ok, stats_map}` (from `stat/2`) or a `stats_map` plain Elixir map (from a successful `stat!/2`), where the key values of `stats_map` are integer byte counts.  Transparent for `{:error, info}` tuples returned from `stat/2`.

  Returns the same structure but with all byte values converted to formatted human-readable strings (e.g., `"10 GiB"`).

  ## Parameters

    * `stats` - either `{:ok, stats_map}` or a `stats_map` with keys like `:available`, `:free`, etc. and integer values representing bytes.
    * `base_type` - formatting base, either `:binary` (default, powers of 1024) or `:decimal` (powers of 1000). Determines the unit suffixes (`KiB` vs `kB`).

  ## Examples

      iex> DiskSpace.humanize({:ok, %{free: 123456789}}, :binary)
      {:ok, %{free: "117.74 MiB"}}

      iex> DiskSpace.humanize(%{total: 1000000}, :decimal)
      %{total: "1 MB"}

      iex> DiskSpace.humanize({:error, :eio}, :binary)
      {:error, :eio}

      iex> DiskSpace.stat("/tmp") |> DiskSpace.humanize

  """
  def humanize(_, base_type \\ :binary)

  def humanize({:ok, stats}, base_type)
      when is_map(stats) and base_type in [:binary, :decimal],
      do: {:ok, humanize(stats, base_type)}

  def humanize(stats, base_type) when is_map(stats) and base_type in [:binary, :decimal],
    do:
      Enum.map(
        stats,
        fn {k, v} -> {k, humanize_bytes(v, base_type)} end
      )
      |> Map.new()

  def humanize({:error, _} = failure, _), do: failure

  defp humanize_bytes(bytes, base_type)
       when is_integer(bytes) and bytes >= 0 and base_type in [:binary, :decimal] do
    binary_units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"]

    units =
      case base_type do
        :binary ->
          binary_units

        :decimal ->
          Enum.map(binary_units, &(String.replace(&1, "i", "") |> String.replace("K", "k")))
      end

    base = if base_type == :binary, do: 1024, else: 1000
    humanize_bytes(bytes, units, base, 0)
  end

  defp humanize_bytes(bytes, [unit | _], base, _exp) when bytes < base,
    do: format_number(bytes) <> " " <> unit

  defp humanize_bytes(bytes, [_ | next_units], base, exp),
    do: humanize_bytes(bytes / base, next_units, base, exp + 1)

  defp format_number(number) when is_integer(number) or is_float(number),
    do: :erlang.float_to_binary(number / 1.0, decimals: 2) |> String.trim_trailing(".00")
end
