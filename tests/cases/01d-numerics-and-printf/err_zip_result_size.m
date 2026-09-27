% covers: 1 - a broadcast result shape is checked before anything is allocated
% The operands are tiny; it is the broadcast of 100000x1 against 1x100000 that
% asks for 1e10 elements. Before this cycle it aborted in the allocator with
% exit 134 and took the REPL with it, so the exit code is the real assertion
% here: 1, never 134.
x = ones(1e5, 1) + ones(1, 1e5);
