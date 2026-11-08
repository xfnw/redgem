{ system ? builtins.currentSystem }:
                    
let      
  pins = import ./npins;
  pkgs = import pins.nixpkgs { inherit system; };
  vulppkgs = import pins.vulppkgs { inherit pkgs; };
in pkgs.mkShell {
  packages = [
    pkgs.cargo
    pkgs.clippy
    pkgs.rustc
    pkgs.rustfmt
    vulppkgs.npinsMinimal
  ];
}
