## MIDL v0.2: changes from proto2/proto3 specification

## Protobuf constraints

Interface files can use protobuf syntax and be authored with `.proto`-aware editors, and even use a `.proto` extension, as long as they adhere to the following constraints:

- syntax = "proto3";
- the following protobuf features cannot be used:
  - types: `fixed`, `sint*`, `oneof`
  - field options
  - reserved fields  
  - `extends` in proto file
  - `option` inside messages or services
  - `import` statements
- package declaration is required  

Note that In messagepack, all message fields are required, even though the proto3 default is that all fields are optional. Protobuf field numbers are ignored.

Use of `.proto`-aware editors has benefits for syntax highlighting and completion, but passing syntax checks in an IDE does not guarantee the file will be parsable by the MIDL parser.

If there is interest in adding support for any of these protobuf features to the parser, please let us know. It may be possible to enable some additional protobuf features add retain compatibility with wasmcloud messaging.


## Other simplifications and additions

MIDL allows some language simplifications and has some additional features, but using any of these will probably result in complaints from protobuf-specific tools.

- primitive types may be used as function parameters or return types (protobuf requires a message type to wrap primitive data types)
- functions may be declared to return nothing (i.e., 'void type in C') using either of the following syntaxes:  
  ```
  // the following two declarations are equivalent
  rpc echo(string) returns ();
  rpc echo(string);
  ```
- Functions may take no parameters, as in `rpc increment()`;
- `int8`, `uint8` can be used as field types
- `float32` and `float64` are aliases for float,double
- `->` is an alias for 'returns'
- `(` and `)` surrounding return type are optional
- the field numbers ( "= n" ) following field name are optional; if missing, the field
  numbers will be generated with an automatic sequence starting at 1. To avoid
  unintentional errors, it is recommended to use all numbered fields or no manually
  numbered fields in the same message. (Note: it's unclear whether we need message
  numbers, and messagepack currently does not use them. These might be removed entirely
  in the future).
- path identifiers such as option names and constant values may use `::` as path
  delimiter instead of `.`.  These are normalized and stored as `.` internally.

### Annotations

`@` lines provide annotations to messages, services, and fields. Annotation syntax 
is more flexible than protobuf `option` and will support some future capabilities.
Valid syntax for annotations:

- `@term`
  - `term` can be any identifier.
- `@term(value)`
  - `term` can be any identifier.
  - `value` can be any constant (int, bool, float, literal string, or an identifier 
    defined previously as a constant).
- `@term(name=value, ...)`
  - `term` can be any identifier. 
  - `name` is an identifier, which must be simple (no namespaces).
  - `value` can be any constant (int, bool, float, literal string, or an identifier 
    defined previously as a constant).
  
Spaces or tabs between terms and punctuation are ignored, but the entire annotation must
be on one line. A semicolon at the end of the line is optional.

The following annotations are currently used by frodobuf:

- `@doc("text")` comment added to generated source code. An item may have multiple 
  `@doc` annotations - each one will create its own comment line in the generated source code.
- `@_source(line=X,col=Y)` These annotations are inserted by the code generator to refer
  to the source location of the definition. 
- `@option(name=value)` encoding for protobuf `option` statements
- `@default(value)` default value for the field. value can be a constant. 
  Currently unimplemented.

### Usage notes

- as in protobuf, multiple types and services may be defined in the same file.
- It is expected that an implementor of a service implements _all_ service methods. In Rust, a 
  frodobuf service generates a Rust trait, so there will be a compiler error if some 
  methods are not implemented. If you intend to declare a service with optional methods,
  declare the optional methods in a separate interface, and in your code, declare which 
  service traits will be handled.
