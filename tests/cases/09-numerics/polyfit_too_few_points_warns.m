% covers: 1 - (Scope, polyfit) polyfit of degree 3 through two points warns with the rank-deficient text of cycle 08's backslash and still returns four coefficients, exit 0
% NOTE: MATLAB warns "Polynomial is not unique; degree >= number of data
% points."; SplatCrab reuses cycle 08's rank_deficient_warning, as the
% spec's Design notes record.
p = polyfit([1 2], [1 2], 3); disp(size(p))
