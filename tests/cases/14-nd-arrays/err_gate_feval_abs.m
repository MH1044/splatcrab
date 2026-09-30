% covers: 12 - feval passes an N-D argument on, so the refusal names the builtin it calls; exit 1
feval(@abs, zeros(2, 2, 2))
