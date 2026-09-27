% covers: fprintf width, precision, left-justify and zero pad, cycling, running out of arguments
fprintf('[%5d]\n', 42);
fprintf('[%-5d]\n', 42);
fprintf('[%05d]\n', 42);
fprintf('[%8.3f]\n', -1.5);
fprintf('[%-8.3f]\n', 1.5);
fprintf('[%08.3f]\n', -1.5);
fprintf('[%.2e]\n', 12345.6789);
fprintf('[%g][%g]\n', 100000, 1000000);
fprintf('[%d %d]\n', [1 2; 3 4]);
fprintf('[%s]=[%d]\n', 'x', 5);
fprintf('%d-%d\n', [1 2 3]);
fprintf('\n');
