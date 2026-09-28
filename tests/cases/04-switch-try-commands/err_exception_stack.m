% covers: 16 - e.stack waits for cycle 05, so it is the dot-indexing error cycle 03 pinned for a value with no fields
try, error('x'), catch e, e.stack, end
