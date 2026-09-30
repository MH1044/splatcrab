% covers: 4 - isequal compares two structs nested 100,000 deep through a field without recursion: equal around 1 and 1, unequal around 1 and 2; exit 0, never a stack overflow (134)
% The struct form of the spec's loop: each pass sets a.f to the chain so
% far and makes a the chain, and a third chain around 2 is built in the
% same loop, so one loop gives both answers.
s = 1; t = 1; u = 2;
for k = 1:100000, a.f = s; s = a; b.f = t; t = b; g.f = u; u = g; end
disp(isequal(s, t))
disp(isequal(s, u))
