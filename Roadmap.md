
## TODO

- testing binary compatibility
  - [x] actor to actor
  - [x] provider to actor (reply)
  - [ ] provider to actor (callback)
  - [ ] actor to provider
    
- Generate other languages
  [ ] AssemblyScript
  [ ] TinyGo
  [ ] C/enscripten


## Under consideration
  
- better error messages for existing .proto files, to help migration
  - option to silently ignore unused protobuf features
  - automatic conversion from .proto?
  - separate converter program protobuf -> midl
  
- MIDL improvements (protobuf compatible)

  - [ ] optional fields and default values
  - [ ] embedded messages
  - [ ] "import" to include another file of type definitions. For now, all types used
    in services should be declared in the same file.
  - [ ] handling identifiers imported from other packages

- MIDL changes - not protobuf compatible - under consideration

  - [ ] define constants at top of file - or in imported file - to be used later in file
  - [ ] support multiple parameters in function call

