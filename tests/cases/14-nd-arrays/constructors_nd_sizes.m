% covers: 1 - the constructors take three or more sizes or a size vector of any length, drop trailing sizes of 1 and make an empty N-D array from a size of 0; true stays logical and every element of rand lies in (0, 1)
z = zeros(2, 3, 4);
disp(size(z))
o = ones([2 2 2]);
disp(size(o))
disp(o(:)')
n = NaN(2, 1, 3);
disp(size(n))
disp(isnan(n(:))')
t = true(1, 1, 3);
disp(size(t))
disp(class(t))
disp(t(:)')
r = rand(2, 2, 2);
disp(size(r))
disp(all(r(:) > 0 & r(:) < 1))
disp(size(zeros(2, 3, 1, 4)))
disp(size(zeros(2, 3, 1)))
disp(size(ones(2, 3, 0)))
disp(size(Inf(1, 2, 2)))
disp(size(false([2 1 2])))
