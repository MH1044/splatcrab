% covers: 1 - true and false take sizes exactly as NaN and Inf do: true(n), true(r, c), true(sz)
% NOTE: true and false return 0/1 doubles until cycle 02 gives them the logical
% NOTE: class, so they are asserted through size and sum, never through disp.
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
