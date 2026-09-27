% covers: 3 - an empty return in an expression is "Too many output arguments."
% The value reaches stdout before the error, so this also tests the flush.
x = disp(3)
