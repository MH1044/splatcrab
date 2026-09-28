% covers: 1 - backslash with a tall matrix is the least-squares solution (Householder QR), no longer square-only
% The least-squares line y = c + m*t through (1, 1), (2, 2) and (3, 2) has
% c = 2/3 and m = 1/2, which %.4f prints as 0.6667 and 0.5000 whatever the
% last bits are. This closes the Known deviations row for square-only
% backslash.
A = [1 1; 1 2; 1 3]; b = [1; 2; 2]; x = A \ b; fprintf('%.4f %.4f\n', x)
