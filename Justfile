doc:
	cargo +nightly rustdoc --open --all-features --target wasm32-unknown-emscripten -- --cfg docsrs

check:
	cargo +nightly check --all-targets --target wasm32-unknown-emscripten

examples:
	mkdir -p out
	RUST_BACKTRACE=1 cargo +nightly build --package em-examples --target wasm32-unknown-emscripten
	emrun out/index.html