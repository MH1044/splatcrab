% covers: 12 - trace goes through math::sum0, so the empty sum is +0 rather than Rust's -0
% Cycle 01c routed sum, mean, norm and dot through sum0 and missed trace, so
% this printed -0.0000. The sign is only visible through a format that shows
% it, which is why the case uses fprintf rather than disp.
fprintf('%.4f\n', trace([]));
