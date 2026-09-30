% covers: 15 - feval passes an N-D argument on to a builtin still behind the gate, so the refusal names kron, the builtin it calls; exit 1
A = reshape(1:24, 2, 3, 4);
feval(@kron, A, 1)
