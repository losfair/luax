# luax

A pure-Rust implementation of Lua. (work in progress)

Built on top of [Hexagon VM](https://github.com/losfair/hexagon).

### Build & Run

Latest nightly version of Rust is needed.

`git clone` both this repository and [luax-bin](https://github.com/losfair/luax-bin) into **the same directory** e.g. `some_directory/luax` and `some_directory/luax-bin`.

Run `cargo build --release` in `luax-bin`. The binary will be at `luax-bin/target/release/luax-bin`.

Currently, luax doesn't include a built-in Lua parser. Instead, it takes an AST file generated by `parser/parse.lua` and `parser/transform.py`, which depend on Python 3, official Lua 5.1, `lua-parser` and `lua-cjson`. See `parser/generate.sh` as an example of how to generate the AST file.

While this project's goal is to support the full Lua language, only basic features are supported at the moment. See `tests/` for things that work.
