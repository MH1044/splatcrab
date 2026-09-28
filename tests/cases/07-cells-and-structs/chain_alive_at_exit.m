% covers: 16 - (Scope, freeing) a chain 250,000 deep through cells and handles, still alive when the script ends, is freed at exit without recursion: exit 0, never a stack overflow (134)
% The size keeps this case well inside the harness timeout on a slower runner; the crash guard
% itself is the unit test deep_chains_through_containers_are_freed_iteratively, on a 2 MB thread.
% The chain of cell_handle_chain_freed, never cleared, so the only free is
% the workspace's own at the end of the run.
c = {}; for k = 1:250000, h = @() c; c = {h}; end; disp(4)
