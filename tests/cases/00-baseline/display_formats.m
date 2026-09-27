% covers: integer, fixed, scientific display; Inf, NaN, empty
% NOTE: MATLAB uses a common scale factor (1.0e+03 *) for non-integer matrices,
% NOTE: prints [NaN Inf -Inf 1] with 1 as an integer, and uses wider integer columns
% NOTE: for values >= 1000. Cycle 02 fixes all of these.
x1 = [1 10 100]
x2 = [-1 2]
x3 = [1.5 2]
x4 = 1e6 * [1 2]
x5 = [1e-5 1]
x6 = [NaN Inf -Inf 1]
x7 = []
x8 = 0.5
