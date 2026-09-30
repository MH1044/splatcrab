% covers: 2 - isequal of cells: equal when of the same size with every pair of elements equal by isequal's rules, the class not compared, a NaN equal to nothing, nested cells and handles included; a cell never equals an array; three arguments alike
% The values are the spec's (S3 and Scope, isequal of cells and structs).
disp(isequal({1, 'a'}, {1, 'a'}))
disp(isequal({1, 'a'}, {1, 'b'}))
disp(isequal({1, 2}, {1; 2}))
disp(isequal({'a'}, {97}))
disp(isequal({NaN}, {NaN}))
disp(isequal({}, {}))
disp(isequal({{1, {2}}}, {{1, {2}}}))
disp(isequal({1}, 1))
disp(isequal({@sin}, {@sin}))
disp(isequal({1, 2}, {1, 2}, {1, 2}))
