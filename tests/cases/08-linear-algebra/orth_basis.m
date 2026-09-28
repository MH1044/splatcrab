% covers: 7 - (Scope, orth) orth(A) is an orthonormal basis of the range, one column per unit of rank
% The range of [1 2; 2 4] is the line through [1; 2], so orth gives one unit
% column along it, of either sign: its projection onto [1; 2] is sqrt(5) in
% absolute value.
O = orth([1 2; 2 4]); disp(size(O)); disp(abs(norm(O) - 1) < 1e-12); disp(abs(abs(O' * [1; 2]) - sqrt(5)) < 1e-12)
