% covers: 1 - true and false take sizes exactly as NaN and Inf do: true(n), true(r, c), true(sz)
% NOTE: written when true and false returned doubles, so they are asserted
% NOTE: through size and sum, never through disp. Cycle 02 made them logical;
% NOTE: its own cases in tests/cases/02-classes-and-display/ pin the class.
disp(size(true(2)))
disp(size(false(2, 3)))
disp(size(true([1 4])))
disp(size(false(-1)))
disp(sum(sum(true(3))))
disp(size(true))
disp(sum(sum(true(2, 3))))
disp(sum(sum(false(3))))
disp(size(false(size(ones(3, 2)))))
disp(size(true(0, 4)))
