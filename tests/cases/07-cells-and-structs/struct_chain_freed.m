% covers: 16 - (Scope, freeing) a struct nested 500,000 deep, each one's field a holding the one before, is freed by clear without recursion: exit 0, never a stack overflow (134)
% The Scope bullet extends the drop worklist to structs as well as cells.
% s.a = s nests by assignment, cheaper per link than a call to struct, which
% keeps the run well inside the harness's time limit.
s = struct(); for k = 1:500000, s.a = s; end; clear s; disp(3)
