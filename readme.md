# Naive implementation of php ls
this project is in pre-alpha stage!!!

##  supported requests
- autocompletion
- hover
- go to definition 
- go to declaration
- references


### Database structure

// Symbol table
kind, name, fqn, implements, implementations,location

// location table
symbol_id, url, range_start_position_line,range_start_position_character, range_end_position_line,range_end_position_character, 


### Indexers
DONE:
- class declaration
- method declaration

@TODO
- class reference
- method referecnce


 we have saved classes and methods
now we need class references and method references.


- once namespace_use_declaration node appears we need to map its name or alias to FQL 
that will allow as to use this map to detect FQN in object creation expression


## GO TO DEFINITION
### Naive version
+1. detect class name under current cursor
+2. scan whole project to find class name declaration node
+3. get location and pass it to response.

@TODO
- Static class calls are not detected as object_creation_expression
we should use scoped_call_expression for this

- run the indexer when ls is started.

