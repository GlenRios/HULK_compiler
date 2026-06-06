@echo off
REM Ejecuta los tests del compilador HULK con el entorno MSYS2 correcto.
REM El PATH de MSYS2 es necesario para que gcc encuentre el ld correcto.

set MSYS2=C:\msys64
set LLVM_SYS_170_PREFIX=C:\llvm17-wrap
set PATH=%MSYS2%\mingw64\bin;%MSYS2%\usr\bin;%PATH%

cargo test --target x86_64-pc-windows-gnu %*
