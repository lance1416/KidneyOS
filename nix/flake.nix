{
  description = "KidneyOS";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # NOTE: rust-overlay only keeps nightly manifests from January 1st of the
    # previous year onwards, so after changing the channel in
    # rust-toolchain.toml you may need to run
    # `nix flake update rust-overlay --flake ./nix`, and the toolchain must be
    # bumped at least every other year for `nix flake update` to keep working.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      # Name of the published builder image (see docs/building/docker.md). The
      # scripts in scripts/*-container.* read it from this line, so keep it on a
      # single line in this form.
      builderImage = "ghcr.io/lance1416/kidneyos-builder";

      # Only Linux hosts are supported: GRUB (needed for grub-mkrescue) is not
      # available on Darwin in nixpkgs. macOS users should use the Docker image
      # or the Homebrew instructions in docs/building/macos.md.
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f {
            inherit system;
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            };
          }
        );
    in
    {
      # Run `nix fmt` from the nix directory to format all Nix files in the
      # repository with the official nixfmt style.
      formatter = forAllSystems ({ pkgs, ... }: pkgs.nixfmt-tree);

      # Run with `nix flake check ./nix`.
      checks = forAllSystems (
        { system, pkgs }:
        let
          # The repository root (only files tracked by Git).
          src = ../.;
          lint =
            name: nativeBuildInputs: script:
            pkgs.runCommand name { inherit nativeBuildInputs; } ''
              cp -r --no-preserve=mode ${src} src
              cd src
              ${script}
              touch $out
            '';
        in
        {
          # Make sure the devShells (and so the builder image's contents)
          # build, not just evaluate.
          devShell-build = self.devShells.${system}.build;
          devShell-default = self.devShells.${system}.default;

          # Equivalent to `nix fmt ./nix -- --ci`.
          nix-fmt = lint "nix-fmt" [ self.formatter.${system} ] ''
            treefmt --ci --tree-root .
          '';

          shellcheck = lint "shellcheck" [ pkgs.shellcheck ] ''
            shellcheck scripts/*.bash
          '';

          actionlint = lint "actionlint" [ pkgs.actionlint pkgs.shellcheck ] ''
            actionlint .github/workflows/*.yaml
          '';
        }
      );

      packages = forAllSystems (
        { system, pkgs }: {
          # A Docker image containing the `build` devShell. It is published by
          # .github/workflows/container.yaml; see also
          # scripts/build-container.bash.
          kidneyos-builder = pkgs.dockerTools.buildNixShellImage {
            name = builderImage;
            tag = "latest";
            drv = self.devShells.${system}.build;
          };
        }
      );

      devShells = forAllSystems (
        { system, pkgs }:
        let
          inherit (pkgs)
            actionlint
            bochs
            gdb
            gnumake
            grcov
            gzip
            mdbook
            mdsh
            mkShell
            mtools
            qemu
            rust-bin
            shellcheck
            xorriso
            xxd
            xz
            ;
          rust = rust-bin.fromRustupToolchainFile ../rust-toolchain.toml;
          # Cross toolchain providing the i686-unknown-linux-gnu-* binutils and
          # a linker for the i686-unknown-linux-gnu tests.
          i686-pkgs = pkgs.pkgsCross.gnu32;
          i686-cc = i686-pkgs.stdenv.cc;
          # grub-mkrescue needs the i386-pc GRUB modules. On x86_64-linux the
          # native GRUB package ships them; elsewhere we run the host's
          # grub-mkrescue but point it at the modules from a cross-built GRUB.
          grub2 =
            if system == "x86_64-linux" then
              pkgs.grub2
            else
              pkgs.runCommand "grub2-mkrescue-i686-cross"
                {
                  nativeBuildInputs = [ pkgs.makeWrapper ];
                }
                ''
                  makeWrapper ${pkgs.grub2}/bin/grub-mkrescue $out/bin/grub-mkrescue \
                    --add-flags "--directory=${i686-pkgs.grub2}/lib/grub/i386-pc"
                '';
        in
        {
          # Changes to this are published to the builder image by
          # .github/workflows/container.yaml once they're merged. See
          # docs/building/docker.md#maintainers for details.
          build = mkShell {
            packages = [
              actionlint
              gnumake
              grcov
              grub2
              gzip
              i686-cc
              mdbook
              mdsh
              mtools
              qemu
              rust
              shellcheck
              xorriso
              xz
            ];

            CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_LINKER = "${i686-cc.targetPrefix}cc";
            CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_RUNNER = "${qemu}/bin/qemu-i386";

            shellHook = ''
              export CARGO_TARGET_DIR="$PWD/build/target"
            '';
          };

          default = self.devShells.${system}.build.overrideAttrs (oldAttrs: {
            nativeBuildInputs = oldAttrs.nativeBuildInputs ++ [
              bochs
              gdb
              xxd
            ];
          });
        }
      );
    };
}
