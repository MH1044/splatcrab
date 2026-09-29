% covers: 5 - fzero with the bracket [0 1] converges to the root of cos(x) = x
% The root 0.73908513... is 4e-7 clear of the 0.7390855 boundary of %.6f.
fprintf('%.6f\n', fzero(@(x) cos(x) - x, [0 1]));
