% covers: 17 - a char range is a char row, and diag of a char row is a char matrix: both keep the class, as MATLAB does
disp('a':'e')
disp(class('a':'c'))
d = diag('ab');
disp(class(d))
disp(size(d))
