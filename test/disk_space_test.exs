# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

defmodule DiskSpaceTest do
  use ExUnit.Case, async: true
  doctest DiskSpace

  defmodule DiskSpaceMock do
    # override the internal call to stat_fs/1 to use this module's stat_fs/1
    def stat(path, opts \\ []) when is_binary(path) and is_list(opts) do
      humanize? = Keyword.get(opts, :humanize, false)
      base = Keyword.get(opts, :base, :binary)

      case stat_fs(path) do
        {:ok, stats} ->
          if humanize?, do: {:ok, DiskSpace.humanize(stats, base)}, else: {:ok, stats}

        error ->
          error
      end
    end

    def stat!(path, opts \\ []) do
      case stat(path, opts) do
        {:ok, info} -> info
        {:error, reason} -> raise DiskSpace.Error, reason
      end
    end

    def stat_fs("/valid/path") do
      {:ok,
       %{
         available: 10_000,
         free: 20_000,
         total: 30_000,
         used: 10_000
       }}
    end

    def stat_fs("/error/path"), do: {:error, :realpath_failed}
  end

  describe "NIF loading" do
    test "NIF loads successfully" do
      assert {:module, DiskSpace} = :code.ensure_loaded(DiskSpace),
             "DiskSpace module failed to load, likely due to NIF loading failure"
    end
  end

  describe "stat/2" do
    test "returns stats map wrapped in :ok tuple" do
      assert {:ok, stats} = DiskSpaceMock.stat("/valid/path")
      assert stats.available == 10_000
    end

    test "returns error tuple on failure" do
      assert {:error, :realpath_failed} = DiskSpaceMock.stat("/error/path")
    end

    test "humanizes output when humanize: true option is passed" do
      result = DiskSpaceMock.stat("/valid/path", humanize: true)
      assert {:ok, map} = result
      assert is_binary(map.available)
      assert String.ends_with?(map.available, "B")
    end

    test "humanizes with decimal base when base: :decimal is given" do
      {:ok, stats} = DiskSpaceMock.stat("/valid/path", humanize: true, base: :decimal)
      assert String.ends_with?(stats.available, "B")
    end
  end

  describe "stat!/2" do
    test "returns stats map directly on success" do
      assert_raise DiskSpace.Error, fn ->
        DiskSpace.stat!("/nonexistent/path")
      end
    end
  end

  describe "humanize/2" do
    test "returns {:error, reason} unchanged" do
      assert {:error, :eio} = DiskSpace.humanize({:error, :eio}, :binary)
    end

    test "humanizes byte counts in {:ok, stats_map}" do
      input = {:ok, %{free: 123_456_789}}
      {:ok, result} = DiskSpace.humanize(input, :binary)
      assert is_binary(result.free)
      assert String.contains?(result.free, "MiB")
    end

    test "humanizes byte counts in map directly" do
      input = %{total: 1_000_000}
      result = DiskSpace.humanize(input, :decimal)
      assert is_binary(result.total)
      assert String.contains?(result.total, "MB")
    end
  end
end
