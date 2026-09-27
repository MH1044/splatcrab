% covers: 14 - diag squares its argument's length, which the same guard checks
% NOTE: 1e5 is small enough to construct, but diag of it asks for 1e10
% NOTE: elements. This is the case where the guard, not the overflow check,
% NOTE: is what keeps the process alive.
diag(zeros(1, 1e5))
