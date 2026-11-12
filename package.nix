{ crane
, lib
, runCommand
, zip
, profile ? "smol"
}:

let
  self = crane.buildPackage {
    src = lib.fileset.toSource {
      root = ./.;
      fileset = lib.fileset.unions [
        ./Cargo.lock
        ./Cargo.toml
        ./src
      ];
    };

    env = {
      CARGO_PROFILE = profile;
      REDGEM_VERSION_INFO = "nix (${placeholder "out"})";
    };

    passthru.withZip = file: runCommand "redgem.zip" {
      nativeBuildInputs = [ zip ];
    } ''
      cp --no-preserve=all ${file} files.zip
      zip -J files.zip
      cat ${self}/bin/redgem files.zip > $out
      zip -A $out
      chmod +x $out
    '';

    meta.mainProgram = "redgem";
  };
in self
