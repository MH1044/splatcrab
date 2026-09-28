% covers: 16 - a cell nested 500,000 deep, c = {c} in a loop, is freed by clear without recursion: exit 0, never a stack overflow (134)
% Freeing goes through cycle 06's drop worklist, extended to cells. disp(1)
% after the clear proves the run went on past the free.
c = {}; for k = 1:500000, c = {c}; end; clear c; disp(1)
