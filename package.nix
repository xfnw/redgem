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

    env.CARGO_PROFILE = profile;

    passthru.withZip = file: runCommand "redgem.zip" {
      nativeBuildInputs = [ zip ];
    } ''
      cp --no-preserve=all ${file} files.zip
      zip -J files.zip
      cat ${self}/bin/redgem files.zip > $out
      zip -A $out
    '';

    meta.mainProgram = "redgem";
  };
in self
