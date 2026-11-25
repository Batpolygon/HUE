@echo off
setlocal
title HUE - build
cd /d "%~dp0"

where npm >nul 2>nul || (echo [HUE] npm introuvable : installe Node.js. & goto :fail)
where cargo >nul 2>nul || (echo [HUE] cargo introuvable : installe Rust ^(rustup^). & goto :fail)

if not exist "node_modules" (
  echo [HUE] Installation des dependances npm...
  call npm install || goto :fail
)

for /f %%v in ('node -p "require('./package.json').version"') do set "VERSION=%%v"
for /f %%m in ('node -p "require('./package.json').version.split('.')[0]"') do set "MAJOR=%%m"
set "FINAL=release\HUE_V%MAJOR%.exe"

tasklist /NH 2>nul | findstr /I /B /L /C:"hue.exe " /C:"HUE_V%MAJOR%.exe " >nul && (
  echo [HUE] HUE.exe ou HUE_V%MAJOR%.exe tourne : ferme-le d'abord
  echo       ^(clic droit sur l'icone ^> Quitter^), sinon l'exe ne peut pas etre remplace.
  goto :fail
)

echo [HUE] Build release v%VERSION% ^(quelques minutes^)...
call npm run build:exe || goto :fail

copy /Y "release\HUE.exe" "%FINAL%" >nul || (echo [HUE] Copie vers %FINAL% impossible. & goto :fail)

echo.
echo [HUE] Build v%VERSION% termine :
echo   release\HUE.exe
if exist "release\HUE_%VERSION%_x64-setup.exe" echo   release\HUE_%VERSION%_x64-setup.exe  ^(installeur^)
echo   %FINAL%
echo.
pause
exit /b 0

:fail
echo.
echo [HUE] Echec du build.
pause
exit /b 1
