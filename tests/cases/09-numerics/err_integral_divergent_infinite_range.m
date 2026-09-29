% covers: 15 - (Scope, invariant 6) integral of 1 over [0, Inf) halves towards the map's pole until no node fits inside, then the subinterval error, exit 1, never an Inf from the pole
% The text is SplatCrab's own, recorded in the spec's Design notes.
% NOTE: MATLAB warns and returns what it has; SplatCrab ends in a clean
% error (Known deviations, "Numerics, cycle 09").
q = integral(@(x) ones(size(x)), 0, Inf);
