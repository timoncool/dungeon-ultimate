@echo off
chcp 65001 >nul
echo Остановка Open Dungeon (порты 3000, 7869, 8080, 8081, 8082)...
for %%P in (3000 7869 8080 8081 8082) do (
  for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":%%P " ^| findstr LISTENING') do taskkill /F /PID %%a >nul 2>&1
)
echo Готово.
timeout /t 2 >nul
