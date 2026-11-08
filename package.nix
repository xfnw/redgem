{ crane
, lib
, profile ? "smol"
}:

crane.buildPackage {
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.lock
      ./Cargo.toml
      ./src
    ];
  };

  env.CARGO_PROFILE = profile;

  meta.mainProgram = "redgem";
}
