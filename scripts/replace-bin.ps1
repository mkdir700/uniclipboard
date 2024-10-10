# 设置脚本在遇到错误时停止执行
$ErrorActionPreference = "Stop"

# 每构建一次，该脚本就会被自动执行一次
Write-Host "Starting replace-bin.ps1"

# 获取构建的二进制文件路径
$builtBinary = Get-ChildItem -Path "dist" -Recurse -Filter "uniclipboard.exe" | Select-Object -First 1 -ExpandProperty FullName

# 获取 artifacts 目录中的二进制文件
$artifactBinary = Get-ChildItem -Path "artifacts" -Recurse -Filter "uniclipboard.exe" | Select-Object -First 1 -ExpandProperty FullName

# 替换构建的二进制文件
Copy-Item -Path $artifactBinary -Destination $builtBinary -Force

Write-Host "二进制文件已成功替换"

Write-Host "Finished replace-bin.ps1"
