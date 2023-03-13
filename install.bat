@echo off

ECHO Building Caveripper
cargo install --path .\cli

ECHO Copying resources folder into $HOME\.config\caveripper\
Xcopy resources %userprofile%\.config\caveripper\ /s /e

ECHO Done. caveripper.exe is located in your cargo/bin directory (usually C:\Users\<Your username>\.cargo\bin)
