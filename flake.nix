{
  description = "Standalone TUI infoview for Lean 4 theorem prover";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    tree-sitter-lean = {
      url = "github:wvhulle/tree-sitter-lean";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      crane,
      tree-sitter-lean,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;
        lib = pkgs.lib;

        src = craneLib.cleanCargoSource ./.;

        # Get the pre-built rust crate source with generated parser.c
        treeSitterLeanSrc = tree-sitter-lean.packages.${system}.rust-crate;

        # Parse Cargo.lock and override tree-sitter-lean revision from flake input
        # This ensures the vendored deps always match the flake input, regardless of Cargo.lock
        cargoLockParsed =
          let
            original = builtins.fromTOML (builtins.readFile ./Cargo.lock);
            tslRev = tree-sitter-lean.rev;
            tslSource = "git+https://github.com/wvhulle/tree-sitter-lean#${tslRev}";
          in
          original
          // {
            package = map (
              p:
              if p.name == "tree-sitter-lean" then
                p // { source = tslSource; }
              else
                p
            ) original.package;
          };

        # Helper to check if a package is tree-sitter-lean
        isTreeSitterLean =
          p: lib.hasPrefix "git+https://github.com/wvhulle/tree-sitter-lean" (p.source or "");

        # Vendor all dependencies, replacing tree-sitter-lean git checkout with our pre-built source
        cargoVendorDir = craneLib.vendorCargoDeps {
          inherit src cargoLockParsed;
          overrideVendorGitCheckout = ps: drv: if lib.any isTreeSitterLean ps then treeSitterLeanSrc else drv;
        };

        # Common arguments for crane builds
        commonArgs = {
          inherit src cargoVendorDir;
          strictDeps = true;
          buildInputs = [ ];
          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        # Build just the cargo dependencies for caching
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate
        lean-tui = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
      {
        packages = {
          default = lean-tui;
          lean-tui = lean-tui;
        };

        checks = {
          inherit lean-tui;
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );
          fmt = craneLib.cargoFmt { src = ./.; };
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs; [
            tree-sitter
            elan
          ];

          shellHook = ''
            echo "lean-tui development shell"
            echo ""

            # Set up .cargo/config.toml to use vendored deps
            mkdir -p .cargo
            if [ ! -f .cargo/config.toml ] || ! grep -q "nix-sources" .cargo/config.toml 2>/dev/null; then
              cat ${cargoVendorDir}/config.toml > .cargo/config.toml
              echo "Configured cargo to use vendored dependencies."
            fi

            echo "Use 'cargo build' to build the project."
            echo "Use 'cargo test' to run tests."
            echo "Use 'nix build' for a reproducible build."
          '';
        };
      }
    );
}
