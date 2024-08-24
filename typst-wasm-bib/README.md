## Compile

To compile this example, you need to have a working [Rust toolchain](https://www.rust-lang.org/). Then we need to install the `wasm32-unknown-unknown` target:

```sh
rustup target add wasm32-unknown-unknown
```

Then, build the crate with this target:

```sh
cargo build --release --target wasm32-unknown-unknown
cp ./target/wasm32-unknown-unknown/release/hello.wasm ./
```

