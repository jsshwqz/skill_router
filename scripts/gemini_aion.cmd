@echo off
setlocal

call "C:\Users\Administrator\AppData\Roaming\npm\gemini.cmd" %* -o text
if %errorlevel%==0 exit /b 0

timeout /t 2 /nobreak >nul
call "C:\Users\Administrator\AppData\Roaming\npm\gemini.cmd" %* -o text
exit /b %errorlevel%
