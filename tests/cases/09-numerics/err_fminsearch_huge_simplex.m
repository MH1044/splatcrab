% covers: Scope, every solver has a cap - fminsearch's simplex is judged by the size check before it is built
% (n + 1) x n values for an n-element start: 1e5 asked for about 80 GB and aborted with 134 before the review fix
x = fminsearch(@(x) 0, zeros(1, 1e5))
