% covers: 6 - mat2str of an integer matrix and of a fraction, sprintf cycling a vector, a width, a precision, the - flag and %s, and fprintf's %%
disp(mat2str([1 2; 3 4]))
disp(mat2str([1.5 2]))
disp(sprintf('%d', [1 2 3]))
disp(sprintf('%5.2f|%-4d|%s', pi, 7, 'ab'))
fprintf('100%%\n')
