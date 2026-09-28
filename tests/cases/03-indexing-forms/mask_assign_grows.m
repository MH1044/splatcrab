% covers: 19 - a mask with a true past the end grows the array on assignment, as a numeric index would
y = [1 2];
y(logical([0 0 1])) = 9;
disp(y)
