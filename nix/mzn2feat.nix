{
  mzn2feat,
  pkgs,
  stdenv,
  ...
}:
let
  runtimeDeps = [
    pkgs.python3
    pkgs.minizinc
  ];
in
stdenv.mkDerivation {
  name = "mzn2feat";
  src = mzn2feat;

  buildInputs = [
    pkgs.flex
    pkgs.bison
    pkgs.makeWrapper
  ];

  buildPhase = ''
    cd fzn2feat
    make
    cd ..
  '';

  installPhase = ''
    mkdir -p $out
    cp -r ./bin $out
  '';

  postFixup = ''
    wrapProgram $out/bin/mzn2feat \
        --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
  '';
}
