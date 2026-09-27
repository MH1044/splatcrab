% covers: 17 - zeros, ones, eye, rand, NaN, Inf, true and false accept a row vector of sizes (QA D21)
% NOTE: all returns a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"). SplatCrab has no logical class until cycle 02
% NOTE: and prints six wide. Adding 0 makes the value a double in both, so the
% NOTE: expected "     1" below is MATLAB's own output, and stays so after 02.
A = ones(2, 3);
disp(size(zeros(size(A))))
disp(size(ones([3 1])))
disp(eye([2 3]))
disp(size(rand([2 3])))
disp(size(NaN([2 3])))
disp(size(Inf([1 4])))
disp(size(true([2 2])))
disp(size(zeros([4])))
disp(size(false(size(A))))
disp(sum(sum(true([2 3]))))
disp(size(zeros([0 3])))
disp(size(zeros(size([]))))
disp(size(ones([2 -1])))
disp(eye([3 2]))
disp(size(eye([4])))
disp(0 + all(all(isnan(NaN([2 3])))))
