run:
	cargo run

test:
	cargo test --features clipboard_tests

ci:
	cargo test --features vendored_openssl

build-all:
	make build-linux
	make build-windows
	make build-macos
	mkdir -p dist
	cp target/release/uniclipboard dist/uniclipboard-macos
	cp target/x86_64-unknown-linux-gnu/release/uniclipboard dist/uniclipboard-linux
	cp target/x86_64-pc-windows-gnu/release/uniclipboard.exe dist/uniclipboard-windows.exe

build-linux:
	cargo build --release --target x86_64-unknown-linux-gnu
	upx --best --lzma target/release/uniclipboard

build-windows:
	cargo build --release --target x86_64-pc-windows-gnu
	upx --best --lzma target/x86_64-pc-windows-gnu/release/uniclipboard.exe

build-macos:
	cargo build --release --target aarch64-apple-darwin
	upx --best --lzma target/aarch64-apple-darwin/release/uniclipboard