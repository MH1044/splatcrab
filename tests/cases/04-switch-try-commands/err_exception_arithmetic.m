% covers: 16 - an MException is not an array, so arithmetic on it is the one refusal the Design notes record
try, error('x'), catch e, e + 1, end
