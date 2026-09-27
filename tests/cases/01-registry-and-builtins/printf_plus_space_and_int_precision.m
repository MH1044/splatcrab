% covers: 20 - printf honours the + and space flags and precision on integers
fprintf('[%+d][% d][%.3d][%+f]\n', 5, 5, 5, 1.5);
fprintf('[%+d][% d]\n', -5, -5);
fprintf('[%+5d][%-+5d][%+05d]\n', 7, 7, 7);
fprintf('[%.5d][%.1d]\n', 42, 42);
fprintf('[% .2f][%+.2f]\n', 1.5, -1.5);
