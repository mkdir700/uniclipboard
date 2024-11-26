set -e
# 每构建一次，该脚本就会被自动执行一次
echo "Starting build-cross-release.sh"

# [ -f artifacts/aarch64-apple-darwin-uniclipboard/uniclipboard ] && mkdir -p dist/darwin_arm64 && cp artifacts/aarch64-apple-darwin-uniclipboard/uniclipboard dist/darwin_arm64
# [ -f artifacts/x86_64-apple-darwin-uniclipboard/uniclipboard ] && mkdir -p dist/darwin_amd64 && cp artifacts/x86_64-apple-darwin-uniclipboard/uniclipboard dist/darwin_amd64
# [ -f artifacts/x86_64-unknown-linux-gnu-uniclipboard/uniclipboard ] && mkdir -p dist/linux_amd64 && cp artifacts/x86_64-unknown-linux-gnu-uniclipboard/uniclipboard dist/linux_amd64
# [ -f artifacts/x86_64-pc-windows-msvc-uniclipboard/uniclipboard.exe ] && mkdir -p dist/windows_amd64 && cp artifacts/x86_64-pc-windows-msvc-uniclipboard/uniclipboard.exe dist/windows_amd64

# • building  binary=dist/uniclipboard_windows_amd64_v1/uniclipboard.exe
# • building  binary=dist/uniclipboard_linux_amd64_v1/uniclipboard
# • building  binary=dist/uniclipboard_darwin_amd64_v1/uniclipboard

# 将提前编译好的二进制文件复制到dist目录下，并重命名为统一的名称，方便后续上传到release中。
[ -f artifacts/x86_64-pc-windows-msvc-uniclipboard/uniclipboard.exe ] &&
  mkdir -p dist/uniclipboard_windows_amd64_v1/ &&
  rm -rf dist/uniclipboard_windows_amd64_v1/uniclipboard.exe &&
  cp artifacts/x86_64-pc-windows-msvc-uniclipboard/uniclipboard.exe dist/uniclipboard_windows_amd64_v1/uniclipboard.exe

[ -f artifacts/x86_64-unknown-linux-gnu-uniclipboard/uniclipboard ] &&
  mkdir -p dist/uniclipboard_linux_amd64_v1/ &&
  rm -rf dist/uniclipboard_linux_amd64_v1/uniclipboard &&
  cp artifacts/x86_64-unknown-linux-gnu-uniclipboard/uniclipboard dist/uniclipboard_linux_amd64_v1/uniclipboard

[ -f artifacts/x86_64-apple-darwin-uniclipboard/uniclipboard ] &&
  mkdir -p dist/uniclipboard_darwin_amd64_v1/ &&
  rm -rf dist/uniclipboard_darwin_amd64_v1/uniclipboard &&
  cp artifacts/x86_64-apple-darwin-uniclipboard/uniclipboard dist/uniclipboard_darwin_amd64_v1/uniclipboard

[ -f artifacts/aarch64-apple-darwin-uniclipboard/uniclipboard ] &&
  mkdir -p dist/uniclipboard_darwin_arm64/ &&
  rm -rf dist/uniclipboard_darwin_arm64/uniclipboard &&
  cp artifacts/aarch64-apple-darwin-uniclipboard/uniclipboard dist/uniclipboard_darwin_arm64/uniclipboard

echo "Finished build-cross-release.sh"
