% covers: 12 - repmat tiles any array along every dimension given, as counts or as a count vector, an N-D array included and its dimensions past the counts kept, trailing counts of 1 dropped, a count of 0 making an empty, and the class kept
A = reshape(1:24, 2, 3, 4);
R = repmat(A, 1, 1, 2);
disp(size(R))
disp(isequal(R(:, :, 1:4), A))
disp(isequal(R(:, :, 5:8), A))
disp(size(repmat([1 2], [2 1 3])))
T = repmat(A, 2, 1);
disp(size(T))
disp(isequal(T(1:2, :, :), A))
disp(isequal(T(3:4, :, :), A))
R = repmat([1 2], 1, 1, 2);
disp(size(R))
disp(R(:)')
disp(size(repmat(1, [2 2 2])))
disp(size(repmat(5, 2, 3, 1)))
disp(size(repmat(5, [2 3 1 1])))
disp(size(repmat(A, [1 1])))
disp(size(repmat(A, 1, 0, 2)))
W = repmat(A, [2 2 2]);
disp(size(W))
disp(isequal(W(3:4, 4:6, 5:8), A))
disp(W(2, 5, 7))
K = repmat([1; 2], [1 2 1 2]);
disp(size(K))
disp(K(:)')
c = repmat('ab', 1, 1, 2);
disp(class(c))
disp(size(c))
disp(c(:)')
disp(class(repmat(true, 2, 2, 2)))
