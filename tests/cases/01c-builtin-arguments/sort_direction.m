% covers: 3 - sort(v, direction), sort(v, dim) and sort(v, dim, direction); descending puts NaN first
% NOTE: a NaN or Inf element still forces a disp row to four decimals
% NOTE: (cycle 01e), so any result holding one is checked through fprintf('%g ').
disp(sort([3 1 2], 'descend'))
fprintf('%g ', sort([3 NaN 1 2], 'descend')); fprintf('\n');
disp(sort([3 1 2], 'ascend'))
disp(sort([3; 1; 2], 'descend')')
disp(sort([3 1 2], 2, 'descend'))
disp(sort([3 1 2], 1))
fprintf('%g ', sort([NaN 1 NaN 2], 'descend')); fprintf('\n');
fprintf('%g ', sort([1 -Inf Inf NaN], 'descend')); fprintf('\n');
fprintf('%g ', sort([1 -Inf Inf NaN], 'ascend')); fprintf('\n');
disp(sort([3 1 2], 2))
disp(sort([3; 1; 2], 1, 'descend')')
disp(sort([3; 1; 2], 2)')
disp(size(sort([3; 1; 2], 'descend')))
