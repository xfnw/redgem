{ system ? builtins.currentSystem
, pins ? import ./npins
, pkgs ? import pins.nixpkgs { inherit system; }
, crane ? import pins.crane { inherit pkgs; }
}:

{
  redgem = pkgs.callPackage ./package.nix { inherit crane; };

  redgem32Static = let
    pkgs' = pkgs.pkgsCross.musl32.pkgsStatic;
  in pkgs'.callPackage ./package.nix {
    crane = import pins.crane { pkgs = pkgs'; };
  };
}
