% covers: 13 - perf guard: 200000 appends with z(end+1) grow in place, never by a copy per append
% The spec asks for "well under one second". The harness has no timer: all it
% enforces is its 10-second TIMEOUT per case, on the debug build. A copy per
% append moves about 2e10 elements and runs far past that; in-place growth
% does not.
% The spec prints the count with disp, as `    200000`, ten wide. Cycle 02's
% integer display rule makes any value of 1000 or more twelve wide, so the two
% disagree; %d keeps this case out of that question. The second line checks
% that the appends stored the right values at both ends.
z = [];
for k = 1:200000, z(end+1) = k; end
fprintf('%d\n', numel(z));
fprintf('%d %d\n', z(1), z(end));
