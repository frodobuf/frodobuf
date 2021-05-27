
This folder contains interface definitions (midl files).


Each directory is structured as followed. The `.midl` file is manually created - all other files are automatically generated. (`Cargo.toml` can be edited after it is generated the first time). Currently only rust language code is being generated.

<br/>

| File | Description  |
| --- | --- |
| `thing/`    | directory for interface 'thing'    |
| &nbsp; ├ `thing.midl` | interface definition file |
| &nbsp; ├ `Cargo.toml` | rust cargo build file |
| &nbsp; ├ `rust/` | directory for generated language code |
| &nbsp; &nbsp; &nbsp; ├ `build.rs` | code to regenerate source when `.midl` changes|
| &nbsp; &nbsp; &nbsp; ├ `src/` | |
| &nbsp; &nbsp; &nbsp; &nbsp; &nbsp; ├ `lib.rs` | library file (loads dynamically-generated source at compile time) |




