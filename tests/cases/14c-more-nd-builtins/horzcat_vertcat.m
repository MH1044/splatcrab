% covers: 9 - horzcat and vertcat are cat along dimensions 2 and 1: N-D arrays joined, a row and a matrix built, an empty beside a nonempty array omitted, the empty the sizes give when every input is empty where the brackets give 0x0, no argument giving [], one argument returned, and the class and complex storage following cat's rules
A = reshape(1:24, 2, 3, 4);
H = horzcat(A, A);
disp(size(H))
disp(isequal(H(:, 4:6, :), A))
V = vertcat(A, A);
disp(size(V))
disp(isequal(V(3:4, :, :), A))
disp(horzcat([1 2], 3))
disp(vertcat([1 2], [3 4]))
disp(size(horzcat(zeros(1, 0), zeros(1, 0))))
disp(size([zeros(1, 0), zeros(1, 0)]))
disp(size(vertcat(zeros(0, 1), zeros(0, 1))))
disp(size(vertcat([1; 2], [])))
disp(size(horzcat(A, [])))
disp(size(horzcat()))
disp(size(vertcat()))
disp(isequal(horzcat(A), A))
disp(horzcat('ab', 'cd'))
disp(class(vertcat(true, false)))
disp(class(horzcat(true, 2)))
disp(isreal(horzcat(1, 2i)))
