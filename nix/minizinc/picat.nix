{
  picat,
  pkgs,
  ...
}:
pkgs.writeText "picat.msc" ''
  {"id": "org.picat-lang.picat", "name": "Picat", "version": "3.9.4", "executable": "${picat}/bin/picat", "mznlib": "", "tags": ["cp", "int"], "supportsMzn": false, "supportsFzn": true, "needsSolns2Out": true, "needsMznExecutable": false, "isGUIApplication": false}
''
