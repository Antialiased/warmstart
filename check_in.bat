del /f /s /q dist
trunk build --release
git add .
git commit -m %0
git push