% covers: 4 - chol of a symmetric matrix that is not positive definite is a clean error, exit 1, after the output before it is flushed
% [1 2; 2 1] has eigenvalues 3 and -1. The text is the spec's recorded
% "Matrix must be positive definite."; the Error: prefix names line 6,
% counting this covers line as line 1.
R = chol([4 2; 2 3]); fprintf('%.4f ', R); fprintf('\n');
chol([1 2; 2 1])
