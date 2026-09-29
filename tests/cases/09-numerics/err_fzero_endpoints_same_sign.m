% covers: 5 - (Scope, fzero) fzero on a bracket whose ends have values of the same sign is a clean error, exit 1
% MATLAB's sentence as recalled, not confirmed against a source; the
% spec's Design notes record it so.
x = fzero(@(x) x^2 + 1, [-1 1]);
