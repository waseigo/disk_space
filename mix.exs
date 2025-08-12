# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

defmodule DiskSpace.MixProject do
  use Mix.Project

  @version "0.1.0"

  def project do
    [
      app: :disk_space,
      version: @version,
      elixir: "~> 1.14",
      compilers: [:elixir_make] ++ Mix.compilers(),
      make: "make -s",
      make_targets: ["all"],
      make_clean: ["clean"],
      make_env: %{"MAKEFLAGS" => "-s"},
      make_cwd: "c_src",
      start_permanent: Mix.env() == :prod,
      description: description(),
      package: package(),
      deps: deps(),

      # Docs
      name: "DiskSpace",
      source_url: "https://github.com/waseigo/disk_space",
      homepage_url: "https://overbring.com/open-source/disk_space",
      docs: [
        main: "readme",
        # logo: "./etc/assets/ex_nominatim_logo.png",
        assets: %{"etc/assets" => "assets"},
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
      files: ["lib", "mix.exs", "README*", "LICENSE*"],
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
      {:elixir_make, "~> 0.9.0", runtime: false},
      {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
      {:ex_doc, "~> 0.38.2", only: :dev, runtime: false}
    ]
  end
end
