{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs-python3-10.url = "github:NixOS/nixpkgs/a71323f68d4377d12c04a5410e214495ec598d4c";
    nixpkgs-uv-6-3.url = "github:NixOS/nixpkgs/3a05eebede89661660945da1f151959900903b6a";
  };
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };
        in
        with pkgs;
        {
          devShells.default = mkShell {
            packages =  [
               nixpkgs-python3-10.python
               nixpkgs-uv-6-3.uv
            ];
            # # 👇 we can just use `rustToolchain` here:
            # buildInputs = [ rustToolchain ];
          };
        }
      );
}