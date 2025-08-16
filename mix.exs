# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

defmodule DiskSpace.MixProject do
  use Mix.Project

  @version "1.0.0"

  def project do
    [
      app: :disk_space,
      version: @version,
      elixir: "~> 1.14",
      # compilers: [:rustler] ++ Mix.compilers(),
      rustler_crates: rustler_crates(),
      start_permanent: Mix.env() == :prod,
      description: description(),
      package: package(),
      aliases: aliases(),
      deps: deps(),

      # Docs
      name: "DiskSpace",
      source_url: "https://github.com/waseigo/disk_space",
      homepage_url: "https://overbring.com/open-source/disk_space",
      docs: [
        main: "readme",
        logo: "./etc/assets/disk_space_logo.png",
        assets: %{"etc/assets" => "etc/assets"},
        extras: ["README.md"]
      ]
    ]
  end

  defp description do
    """
    Returns total, used, free and available bytes for a path on disk using NIFs.
    """
  end

  defp package do
    [
      files: ~w(
        lib
        .formatter.exs
        mix.exs
        README.md
        LICENSE
        native*
        Cargo.toml
        etc*
      ),
      maintainers: ["Isaak Tsalicoglou"],
      licenses: ["Apache-2.0"],
      links: %{"GitHub" => "https://github.com/waseigo/disk_space"}
    ]
  end

  def application do
    [
      extra_applications: []
    ]
  end

  defp deps do
    [
      {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
      {:ex_doc, "~> 0.38.2", only: :dev, runtime: false},
      {:rustler, "~> 0.36.2", runtime: false}
    ]
  end

  defp rustler_crates do
    [
      disk_space: [
        path: "native/diskspace",
        mode: if(Mix.env() == :prod, do: :release, else: :debug)
      ]
    ]
  end

  defp aliases do
    [
      fmt: [
        "format",
        "cmd cargo fmt --manifest-path native/diskspace/Cargo.toml"
      ]
    ]
  end
end
