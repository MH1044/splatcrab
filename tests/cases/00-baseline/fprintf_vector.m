% covers: fprintf format cycling over a vector, width and precision, escapes
fprintf('%6.3f\n', linspace(0, 1, 3));
fprintf('%d %d\n', [1 2 3 4]);
fprintf('%-5d|%05.1f|%s|%e\n', 7, 3.27, 'ab', 12345.6789);
fprintf('100%%\n');
fprintf('%g %g %g\n', 0.0001, 1000000, 1.5);
