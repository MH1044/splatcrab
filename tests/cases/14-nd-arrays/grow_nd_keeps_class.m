% covers: 6 - a char and a logical grown into a second page keep their class, the logical's new element false
c = 'ab';
c(:, :, 2) = 'cd';
disp(class(c))
disp(size(c))
disp(c(:, :, 2))
L = true(1, 2);
L(1, 1, 2) = 1;
disp(class(L))
disp(size(L))
disp(L(:)')
