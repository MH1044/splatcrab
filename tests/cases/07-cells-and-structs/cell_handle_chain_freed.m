% covers: 16 - a chain 250,000 deep alternating cells and handles, each handle capturing the cell before it, is freed by clear without recursion: exit 0, never a stack overflow (134)
% The size keeps this case well inside the harness timeout on a slower runner; the crash guard
% itself is the unit test deep_chains_through_containers_are_freed_iteratively, on a 2 MB thread.
% A cell holds a handle and the handle captures a cell, so the worklist must
% cross both kinds. disp(2) after the clear proves the run went on.
c = {}; for k = 1:250000, h = @() c; c = {h}; end; clear c h; disp(2)
