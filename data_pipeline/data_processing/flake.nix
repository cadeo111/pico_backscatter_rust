{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = {
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem
    (
      system: let
        pkgs = import nixpkgs {
          inherit system;
        };
      in
        with pkgs; {

          
          programs.nix-ld.enable = true;

          devShells.default = mkShell {

            # so that pycharm / jupyter knows where c++ libs are
             LD_LIBRARY_PATH = "${pkgs.stdenv.cc.cc.lib}/lib";

            name = "data_processing";

            packages = [
              python3
              uv
              gcc
            ];

            shellHook = ''
              echo installing uv deps
              uv sync
              source .venv/bin/activate
              alias start='pycharm-professional .;exit'

            '';
          };
        }
    );
}
