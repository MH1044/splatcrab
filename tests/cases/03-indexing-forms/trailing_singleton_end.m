% covers: 15 - end is 1 in a third subscript position, so A(2, 1, end) is A(2, 1, 1)
% From the Scope's trailing-singleton bullet; no Acceptance item spells it out.
% An end of 2 there would be the position-3 bounds error instead.
A = [1 2; 3 4];
disp(A(2, 1, end))
