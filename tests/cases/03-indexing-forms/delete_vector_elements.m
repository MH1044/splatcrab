% covers: 4 - x(i) = [] deletes from a vector by position and by logical mask
x = 1:5;
x(2) = [];
disp(x)
x(logical([1 0 1 0])) = [];
disp(x)
