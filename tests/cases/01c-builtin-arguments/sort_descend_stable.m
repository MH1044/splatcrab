% covers: 3 - sort is stable in both directions: equal elements keep their input order, and descending puts NaN first
% NOTE: -0 and 0 are equal elements, so 1 ./ x (-Inf for -0, Inf for 0) shows
% NOTE: their order. The MATLAB sort page: stable "regardless of sorting
% NOTE: direction". Reversing the ascending sort would flip the zeros. Octave
% NOTE: 8.4 gives every line below. The second output [s, i] = sort(...),
% NOTE: which would show the same through indices, is cycle 03's.
x = sort([-0 0 -0], 'descend'); fprintf('%g ', 1 ./ x); fprintf('\n');
x = sort([0 -0 0], 'descend'); fprintf('%g ', 1 ./ x); fprintf('\n');
x = sort([-0 0 -0], 'ascend'); fprintf('%g ', 1 ./ x); fprintf('\n');
x = sort([-0 2 0 2 -0], 'descend'); fprintf('%g ', 1 ./ x); fprintf('\n');
x = sort([0 NaN -0], 'descend'); fprintf('%g ', 1 ./ x); fprintf('\n');
x = sort([0; NaN; -0], 1, 'descend'); fprintf('%g ', 1 ./ x); fprintf('\n');
