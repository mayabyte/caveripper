@echo off

ECHO Building Caveripper
cargo install --path .\cli

ECHO Copying resources folder into $HOME\.config\caveripper\
ECHO If prompted to overwrite, answer "All"
Xcopy resources %userprofile%\.config\caveripper\resources\ /s /e

ECHO Done. caveripper\.exe is located in your cargo/bin directory (usually C:\Users\<Your username>\.cargo\bin)
