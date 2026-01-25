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

        # Get the tree-sitter-lean grammar with generated parser
        treeSitterLean = tree-sitter-lean.packages.${system}.default;

        # Common arguments for crane builds
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
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
            # tree-sitter CLI for debugging/testing parse trees
            tree-sitter
            elan
          ];

          TREE_SITTER_LEAN_PATH = treeSitterLean;

          shellHook = ''
            echo "lean-tui development shell"
            echo ""
            echo "tree-sitter-lean grammar: $TREE_SITTER_LEAN_PATH"
            echo ""
            echo "Commands:"
            echo "  cargo build    - Build the project"
            echo "  cargo test     - Run tests"
            echo "  cargo run      - Run lean-tui"
            echo "  nix build      - Build with Nix"
            echo "  nix flake check - Run checks (clippy, fmt)"
          '';
        };
      }
    );
}
