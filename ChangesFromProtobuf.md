## MIDL v0.1: changes from proto2/proto3 specification

### Field types

- removed fixed and sint*
- added int8, uint8
- added float32, float64 as aliases for float, double
- removed field option '[]' syntax (see [Annotations](#Annotations))

### Keywords

- removed 'stream' (proto2)
- removed 'oneof'  (might bring this back in the future)
- removed 'syntax'
- removed 'reserved' fields
- removed 'extends' in proto file, but still allowed in messages
- removed 'options' inside message or service. (see [Annotations](#Annotations))
   
### Optional Fields

- The use of 'optional' and 'required' changed between proto2 and proto3.
  Frodobuf's implementation is closer to that of proto2:
  - all fields, unless otherwise annotated, are required.
  - fields can be declared optional either with the 'optional' keyword (before the type)
    or '?' (after the identifier, before the semicolon)
  - the 'required' keyword is accepted but has no effect
  - a default value annotation will be implemented in the future
  - note: optional fields and default value annotations are not fully implemented, 
    and their behavior is undefined.
   
### Comments

C-style comments (`// .. ` or `/* ... */` ) are documentation for the midl file only. Use
one or more `@doc` annotations for service, message, and fields, for documentation that 
should be added to generated source code. This distinction make it possible to comment
out parts of the schema.

### Annotations

'@' lines provide annotations to messages, services, and fields. Annotation syntax 
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

The following are currently used by frodobuf:

- `@doc("text")` comment added to generated source code. An item may have multiple 
  `@doc` annotations - each one will create its own comment line in the generated source code.
- `@_source(line=X,col=Y)` These annotations are inserted by the code generator to refer
  to the source location of the definition. 
- `@option(name=value)` encoding for protobuf `option` statements
- `@default(value)` default value for the field. value can be a constant. 
  Currently unimplemented.


### Other

- a `package` declaration is required.
- as in protobuf, multiple types and services may be defined in the same file.
- It is expected that an implementor of a service implements all methods. In Rust, a 
  frodobuf service generates a Rust trait, so there will be a compiler error if some 
  methods are not implemented. If you intend to declare a service with optional methods,
  declare the optional methods in a separate interface, and in your code, declare which 
  service traits will be handled.
- added "->" as an alias for "returns"
- void function returns can be denoted either by `returns ()` or by omitting returns 
  entirely. For example, 
```protobuf
  // the following two declarations are equivalent
  rpc echo(string) returns ();
  rpc echo(string);
```
- Functions may take no parameters, as in 
```protobuf
  rpc increment();
```
- the field numbers ( "= n" ) following field name are optional; if missing, the field 
  numbers will be generated with an automatic sequence starting at 1. To avoid 
  unintentional errors, it is recommended to use all numbered fields or no manually 
  numbered fields in the same message. (Note: it's unclear whether we need message 
  numbers, and messagepack currently does not use them. These might be removed entirely 
  in the future).
- path identifiers such as option names and constant values may use "::" as path 
  delimiter instead of ".".  These are normalized and stored as "." internally.
