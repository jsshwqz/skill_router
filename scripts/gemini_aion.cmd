@echo off
setlocal

set "HTTP_PROXY=http://127.0.0.1:10808"
set "HTTPS_PROXY=http://127.0.0.1:10808"
set "NO_PROXY=localhost,127.0.0.1,::1"

set "GEMINI_MODEL="
set "GOOGLE_MODEL="
set "GOOGLE_GENERATIVE_AI_MODEL="
set "GOOGLE_GENAI_MODEL="
set "VERTEX_MODEL="
set "GOOGLE_API_KEY="
set "GEMINI_API_KEY="
set "MODEL="

call "C:\Users\Administrator\AppData\Roaming\npm\gemini.cmd" %* -o text
if %errorlevel%==0 exit /b 0

timeout /t 2 /nobreak >nul
call "C:\Users\Administrator\AppData\Roaming\npm\gemini.cmd" %* -o text
exit /b %errorlevel%
