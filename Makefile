run:
	cargo run

build-linux:
	cargo build --release
	upx --best --lzma target/release/uniclipboard

build-windows:
	cargo build --release --target x86_64-pc-windows-gnu
	upx --best --lzma target/x86_64-pc-windows-gnu/release/uniclipboard.exe