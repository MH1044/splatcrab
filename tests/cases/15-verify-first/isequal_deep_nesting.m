% covers: 4 - isequal compares two cells nested 100,000 deep without recursion: equal around 1 and 1, unequal around 1 and 2; exit 0, never a stack overflow (134)
% The spec's loop, with a third chain around 2 built in the same loop, so
% one loop gives both answers. The struct form of this item is
% isequal_deep_nesting_structs.
c = 1; d = 1; e = 2;
for k = 1:100000, c = {c}; d = {d}; e = {e}; end
disp(isequal(c, d))
disp(isequal(c, e))
