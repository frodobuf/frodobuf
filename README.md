# Frodobuf

Experimental approach for defining interfaces for 
[wasmcloud](https://wasmcloud.dev) actors and capability providers.
This project arose out of an interest to experiment with code generation,
to see if both performance and ergonomics could be improved. 
The input syntax is based on a subset of protobuf:

> **Frodobuf** - a little smaller, but it gets the job done

The term "**Frodobuf**" refers to this project, the set of related crates,
the code generator, and the serialization format. 

The term **MIDL** or (**midl**) is used to describe the input 
interface definition (idl) file and the idl file format.

Frodobuf generates code that is binary-compatible with wapc.
Any incompatibilities with wasmcloud host, existing actors,
or existing capability providers should be reported as a bug.

Supported output languages:

- [X] Rust
 
That's all so far.

## In this repo

- **frodobuf**: the main library containing code generation and runtime message processing. 
- **frodobuf-derive**: derive macros for rust code generation. imported indirectly, through the frodobuf library crate.
- **frodobuf-schema**: the schema model - intermediate representation between parser and codegen
- **interfaces**: sample idl files and corresponding generated code
- **midl**: command-line tool to invoke parser and code generator. This cli is a thin wrapper around the frodobuf library.
- **midl-parser**: generate schema from protobuf idl files


## Getting Started

Install the MIDL cli tool with  `cargo install midl`. That's the only program you need to
parse midl, generate schemas, and perform code generation for any supported output 
language.

### Sample code

There are some sample actors and providers in the 
[frodobuf-examples](https://github.com/frodobuf/frodobuf-examples) repository. 
Each of the example interfaces there has both a `.midl` 
and a `.proto` syntax definition file, which produce equivalent schemas.

### Creating a new interface library

The code generator reads a `midl` file and creates a library that can be shared 
or linked for building a "client" (caller) or "server" (handler). 
To create a new interface library, create a folder and add `interface.midl`,
where `interface` is the interface name. In that folder, run 

  `midl create -i interface.midl -l rust`

This command creates `rust/Cargo.toml`,
`rust/build.rs`, and `rust/src/lib.rs`
You can edit the package name in Cargo.toml if you wish, and then `cargo build`.

The `-l` parameter specifies the output language. 
At the moment, only rust generation is supported, but we'd like.
_PR contributions for other target languages are welcome!

After the code is generated, any changes to the midl file will cause the rust sources 
to be regenerated automatically with `cargo build` in that folder 
or rebuilding any project that depends on the interface library.

### ...but where's the generated code?

The `midl create ...` command creates _some_ source files, but the files
generated by that command don't reflect the contents of the original
MIDL file. An additional source file, `interface.rs` is generated 
in a temporary build folder (`target/debug/build/.../interface.rs`).
_Every_ time the `midl` file changes, this file is regenerated,
ensuring that the source code is always up-to-date with the interface
definition. Due to a restriction of
`crates.io`, dynamically generated source code cannot be put 
into the `src` folder, which is why it's generated in this hard-to-find place.
If you're curious to see the interface file generated from the library,
and don't want to hunt for it, 
you can run `midl update -i interface.midl -l rust` and it will put the
`interface.rs` file into the current directory.

To minimize the amount of code you have to write by hand,
the frodobuf rust language api uses some derive macros to generate some 
additional code, including a dispatch function that routes incoming messages
to the service handlers (trait implementation) in your application. 
To see the code generated by macro expansion, cd to the directory
containing Cargo.toml and run `cargo expand > file.rs`. Some IDEs have 
an option to view the results of macro expansion in the IDE.

### Additional Documentation

- [Changelog](./CHANGELOG.md) - summary of recent changes
- [ChangesFromProtobuf](ChangesFromProtobuf.md) - how MIDL differs from proto2/proto3 specification
- [Roadmap](Roadmap.md) - roadmap items - todo's and items under consideration


## How is this different from widl/wapc?

1. The source language (MIDL) is based on a subset of protobuf. See [Changes from protobuf](ChangesFromProtobuf.md) for details.

2. Intermediate Schema

   MIDL files are parsed to generate a frodobuf `Schema` (defined in the `frodobuf-schema` crate), an intermediate representation that is independent of the MIDL, and independent of the generated output language, message encoding, serialization, and message transport. The `Schema` abstraction makes it possible to connect different source IDL languages with different code generator implementations,

   You can view the schema with `midl -i interface.midl -o output.json [ --pretty ]`. 

3. Schema id

   Each service has a unique identifier that is globally unique for the service and for the interface version. The schema id is a base64-encoded sha256 hash of a normalized version of the service definition. Any changes to the schema definition, such as changes in method names, type structs, or even documentation strings, will result in a different schema id.

   A service's schema id and its serialized schema can be accessed at runtime and can be used for reflection and interface discovery.

4. Tooling

   All the tools are in Rust - no nvm or nodejs required.

5. Improvements in Rust code generation

  - no unwrap() calls - fewer chances of panic occurring at runtime for malformed messages
  - apis for sending and receiving messages are (nearly) the same, whether source or dest are actor or provider (this might be just for rust - haven't tried the other languages yet)
  - client and server stubs form a shared "interface" library
    - keeps client and server in sync using same serialization/deserialization
    - greater amount of code generated
  - use of Cow to hold messages to reduce need for copying
  - message parameters are references, to reduce the need for copying  
  - All generated apis are async. Even though wasm is currently single-threaded, async apis are still useful in wasm, and threading may come someday
  - rpc methods are grouped into traits (based on a protobuf service definition)
    - trait enables compile-time parameter checking, ide auto-completion, etc.
    - all senders and receivers, actor and provider, implement the same trait
  - "registration" is done with macros. No hashtables, just simple string lookup. This reduces the amount of manual code required for handler classes.
  - optional generation of full rust crate with Cargo.toml & build.rs

  The frodobuf crate and the wasmcloud-system-interface crate
  replace wapc-guest and wasmcloud-actor-core

