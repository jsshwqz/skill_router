@echo off
setlocal

set "HTTP_PROXY=http://127.0.0.1:10808"
set "HTTPS_PROXY=http://127.0.0.1:10808"
set "NO_PROXY=localhost,127.0.0.1,::1"

set "ANTHROPIC_AUTH_TOKEN="
set "ANTHROPIC_API_KEY="
set "ANTHROPIC_BASE_URL="
set "ANTHROPIC_MODEL="
set "ANTHROPIC_DEFAULT_OPUS_MODEL="
set "ANTHROPIC_DEFAULT_SONNET_MODEL="
set "ANTHROPIC_DEFAULT_HAIKU_MODEL="
set "ANTHROPIC_REASONING_MODEL="

call "C:\Users\Administrator\AppData\Roaming\npm\claude.cmd" %* --model sonnet --no-session-persistence --disable-slash-commands
exit /b %errorlevel%
