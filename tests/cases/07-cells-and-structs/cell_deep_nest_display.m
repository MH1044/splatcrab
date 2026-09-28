% covers: 3 - (Scope, displays) a cell nested 100,000 deep displays one level, its element summarised as {1x1 cell} and never expanded: exit 0, never a stack overflow (134)
% The spec's Design notes: a cell inside a cell is {1×1 cell} and is never
% expanded, which is the bound on displaying a deep nest. The chain is
% then freed at exit through the drop worklist.
c = {}; for k = 1:100000, c = {c}; end
c
