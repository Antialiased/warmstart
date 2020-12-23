del /f /s /q dist
trunk build --release
git add .
git commit -m %1
git push