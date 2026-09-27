% covers: 11 - an infinite range end point is refused rather than quietly giving a 1x0
% `range` used to return early on a non-finite end point, before the count was
% taken, so this produced an empty and exit 0. The count is Inf, and a shape of
% 1xInf is what check_shape already refuses for zeros(1, Inf).
% NOTE: 1:NaN is deliberately not tested. Octave gives a 1x1 NaN there and
% MATLAB is unverified, so that case stays recorded rather than pinned.
0:Inf
