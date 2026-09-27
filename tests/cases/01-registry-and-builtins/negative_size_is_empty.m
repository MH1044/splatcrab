% covers: 24 - a negative size argument is 0, as in MATLAB, not an error
disp(size(zeros(-1)))
disp(size(zeros(2, -3)))
disp(size(ones(-2, 3)))
disp(size(rand(-1, 4)))
disp(size(eye(-2)))
disp(isempty(zeros(-1)))
