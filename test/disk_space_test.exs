# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

defmodule DiskSpaceTest do
  use ExUnit.Case, async: true
  doctest DiskSpace

  describe "NIF loading" do
    test "NIF loads successfully" do
      assert {:module, DiskSpace} = :code.ensure_loaded(DiskSpace),
             "DiskSpace module failed to load, likely due to NIF loading failure"
    end
  end

  describe "stat/2" do
    test "returns stats map wrapped in :ok tuple for valid directory" do
      path = valid_directory_path()
      assert {:ok, stats} = DiskSpace.stat(path)
      assert is_map(stats)
      assert Enum.sort(Map.keys(stats)) == [:available, :free, :total, :used]
      assert is_integer(stats.available)
      assert is_integer(stats.free)
      assert is_integer(stats.total)
      assert is_integer(stats.used)
      assert stats.total >= stats.free
      assert stats.total >= stats.used
    end

    test "returns error tuple for non-existent path" do
      path = Path.join(valid_directory_path(), "nonexistent_#{System.unique_integer()}")
      assert {:error, %{reason: reason, info: info}} = DiskSpace.stat(path)

      assert reason in [
               :not_directory,
               :invalid_path,
               :winapi_failed,
               :statvfs_failed,
               :statfs_failed
             ]

      assert is_map(info) or is_nil(info)

      if is_map(info) do
        assert Map.has_key?(info, :errno)
        assert Map.has_key?(info, :errstr)
      end
    end

    test "returns error tuple for non-directory path" do
      file_path = Path.join(valid_directory_path(), "testfile_#{System.unique_integer()}.txt")
      File.write(file_path, "test")
      assert {:error, %{reason: reason, info: info}} = DiskSpace.stat(file_path)
      assert is_map(info) or is_nil(info)
      assert is_atom(reason)

      if is_map(info) do
        assert Map.has_key?(info, :errno)
        assert Map.has_key?(info, :errstr)
      end

      File.rm(file_path)
    end

    test "humanizes output when humanize: non-nil option is passed" do
      path = valid_directory_path()
      assert {:ok, stats} = DiskSpace.stat(path, humanize: :binary)
      assert is_map(stats)
      assert Enum.sort(Map.keys(stats)) == [:available, :free, :total, :used]
      assert is_binary(stats.available)
      assert String.match?(stats.available, ~r/^[0-9.]+ [KMGTP]?i?B$/)
      assert String.match?(stats.free, ~r/^[0-9.]+ [KMGTP]?i?B$/)
      assert String.match?(stats.total, ~r/^[0-9.]+ [KMGTP]?i?B$/)
      assert String.match?(stats.used, ~r/^[0-9.]+ [KMGTP]?i?B$/)
    end

    test "humanizes with decimal base when base: :decimal is given" do
      path = valid_directory_path()
      assert {:ok, stats} = DiskSpace.stat(path, humanize: :decimal)
      assert is_map(stats)
      assert Enum.sort(Map.keys(stats)) == [:available, :free, :total, :used]
      assert is_binary(stats.available)
      assert String.match?(stats.available, ~r/^[0-9.]+ [kMGTP]?B$/)
      assert String.match?(stats.free, ~r/^[0-9.]+ [kMGTP]?B$/)
      assert String.match?(stats.total, ~r/^[0-9.]+ [kMGTP]?B$/)
      assert String.match?(stats.used, ~r/^[0-9.]+ [kMGTP]?B$/)
    end
  end

  describe "stat!/2" do
    test "returns stats map directly on success" do
      path = valid_directory_path()
      stats = DiskSpace.stat!(path)
      assert is_map(stats)
      assert Enum.sort(Map.keys(stats)) == [:available, :free, :total, :used]
      assert is_integer(stats.available)
      assert is_integer(stats.free)
      assert is_integer(stats.total)
      assert is_integer(stats.used)
      assert stats.total >= stats.free
      assert stats.total >= stats.used
    end

    test "raises DiskSpace.Error on failure" do
      path = Path.join(valid_directory_path(), "nonexistent_#{System.unique_integer()}")

      assert_raise DiskSpace.Error, fn ->
        DiskSpace.stat!(path)
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

  defp valid_directory_path do
    if :os.type() == {:win32, :nt} do
      "C:\\"
    else
      "/tmp"
    end
  end
end
