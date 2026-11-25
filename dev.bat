@echo off
setlocal
title HUE - dev
cd /d "%~dp0"

where npm >nul 2>nul || (echo [HUE] npm introuvable : installe Node.js. & goto :fail)
where cargo >nul 2>nul || (echo [HUE] cargo introuvable : installe Rust ^(rustup^). & goto :fail)

if not exist "node_modules" (
  echo [HUE] Installation des dependances npm...
  call npm install || goto :fail
)

tasklist /NH 2>nul | findstr /I /B /L /C:"hue.exe " /C:"hue_v" >nul && (
  echo [HUE] Attention : une autre instance de HUE tourne deja.
  echo       Ferme-la ^(clic droit sur l'icone ^> Quitter^) pour eviter deux pets a l'ecran.
  echo.
)

echo [HUE] Lancement en mode dev ^(Ctrl+C pour arreter^)...
call npm run tauri dev
if errorlevel 1 goto :fail
exit /b 0

:fail
echo.
echo [HUE] Echec.
pause
exit /b 1
