del /f /s /q dist
cp SwishScope.ico dist\SwishScope.ico
trunk build --release
git add .
git commit -m %1
git push