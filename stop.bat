@echo off
chcp 65001 >/dev/null
echo Остановка Open Dungeon (порты 3000, 7869, 8080)...
for %%P in (3000 7869 8080) do (
  for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":%%P " ^| findstr LISTENING') do taskkill /F /PID %%a >/dev/null 2>&1
)
echo Готово.
timeout /t 2 >/dev/null
