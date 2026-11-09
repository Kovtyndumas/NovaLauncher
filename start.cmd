@echo off
cd /d "%~dp0"
cargo run --release --locked
pause
