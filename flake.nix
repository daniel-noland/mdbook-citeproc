{
  description = "mdbook-citeproc let's pandoc handle bibs for mdbook";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        craneLib = crane.lib.${system};
        mdbook-citeproc = craneLib.buildPackage {
          src = craneLib.cleanCargoSource (craneLib.path ./.);
          strictDeps = true;
          buildInputs = [];
        };
      in
      {
        checks = {
          inherit mdbook-citeproc;
        };

        packages.default = mdbook-citeproc;

        apps.default = flake-utils.lib.mkApp {
          drv = mdbook-citeproc;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [
          ];
        };
      });
}