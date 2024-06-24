doc:
	cargo +nightly rustdoc --open --all-features --target wasm32-unknown-emscripten -Zbuild-std -- --cfg docsrs

examples:
	cargo +nightly build --release --package examples --target wasm32-unknown-emscripten -Zbuild-std
	# emrun --no-browser