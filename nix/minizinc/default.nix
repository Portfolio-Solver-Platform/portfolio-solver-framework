{
  pkgs,
  stdenv,
  ...
}:
let
  runtimeDeps = [
    # Solvers
    pkgs.highs
    pkgs.scipopt-scip # TODO: Handle Apache 2.0 license
    # pkgs.picat # TODO: Look into Mozilla public license 2.0
  ];
  solver_path = "/share/minizinc/solvers";

  picatMsc = pkgs.callPackage ./picat.nix { picat = pkgs.picat; };
in
stdenv.mkDerivation {
  name = "minizinc";
  src = ./.;

  buildInputs = [
    pkgs.makeWrapper
  ];

  buildPhase = "";

  installPhase = ''
    mkdir -p $out/bin
    cp -r ${pkgs.minizinc}/bin/* $out/bin/

    mkdir -p $out${solver_path}
    cp ${picatMsc} $out${solver_path}
  '';
  # cp -r ./solvers/* $out${solver_path}

  postFixup = ''
    wrapProgram $out/bin/minizinc \
        --prefix MZN_SOLVER_PATH : "$out${solver_path}" \
        --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeDeps} \
        --set MZN_STDLIB_DIR "${pkgs.minizinc}/share/minizinc"
  '';
}
