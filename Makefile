run:
	cargo run

build:
	cargo build --release
	upx --best --lzma target/release/uniclipboard
