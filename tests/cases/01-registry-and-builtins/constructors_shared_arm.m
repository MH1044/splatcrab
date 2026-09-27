% covers: 7 - zeros, ones, eye and rand share one arm and still dispatch correctly
disp(zeros(2, 3))
disp(size(zeros(2, 3)))
disp(ones(2))
disp(eye(3))
disp(size(rand(2, 2)))
disp(all(all(rand(2, 2) < 1)))
