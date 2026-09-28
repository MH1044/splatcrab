% covers: 17 - zeros, ones, eye, rand, NaN, Inf, true and false accept a row vector of sizes (QA D21)
% NOTE: all returns a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"), as it has been here since cycle 02. Adding 0
% NOTE: makes the value a double, so the expected "     1" below is the double
% NOTE: width, in MATLAB and here alike.
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
