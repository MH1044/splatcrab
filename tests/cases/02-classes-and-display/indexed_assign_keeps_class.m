% covers: 6 - indexed assignment keeps the left-hand side's class
% Assigning 5 into a logical stores true; assigning 'a' into a double stores 97.
x = true(1,3);
x(2) = 5;
disp(class(x))
disp(x)
y = [1 2 3];
y(2) = 'a';
disp(y)
