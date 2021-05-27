
This folder contains interface definitions (midl files).


Each directory is structured as followed. The `.midl` file is manually created - all other files are automatically generated. (`Cargo.toml` can be edited after it is generated the first time). Currently only rust language code is being generated.

<br/>






| File | Description  |
| --- | --- |
| `thing/`    | directory for interface 'thing'    |
| &nbsp; ├ `thing.midl` | interface definition file |
| &nbsp; ├ `rust/` | directory for generated language code |
| &nbsp; &nbsp; &nbsp; ├ `Cargo.toml` |  |
| &nbsp; &nbsp; &nbsp; ├ `build.rs` | code to rebuild source when `.midl` changes|
| &nbsp; &nbsp; &nbsp; ├ `src/` | |
| &nbsp; &nbsp; &nbsp; &nbsp; &nbsp; ├ `lib.rs` | placeholder that includes dynamically-generated source (see below)  |
| &nbsp; &nbsp; &nbsp; ├ `target/.../thing.rs` | dynamically-generated source file corresponding to the midl |




<br/>
Any updates to the idl file will result in regeneration of the `thing.rs` file. This file is usually not saved in git. 
If you want to 



The reason these files are not generated in the src/ directory is because there is a rule that for published crates
on crates.io, no files inside the crate source tree may be modified.
              


