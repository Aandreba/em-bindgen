doc:
	cargo +nightly rustdoc --open --all-features --target wasm32-unknown-emscripten -- --cfg docsrs

examples:
	cargo +nightly build --package em-examples --target wasm32-unknown-emscripten
	node target/wasm32-unknown-emscripten/debug/em-examples.js