% covers: 22 - %s of a number is its character, and a char argument expands per character
% NOTE: the expansion is per conversion, not per argument: %s still consumes a
% NOTE: whole char argument, which is what makes sprintf('%s %s', 'ab', 'cd')
% NOTE: read as "ab cd" in MATLAB.
fprintf('[%s]\n', 65);
fprintf('[%d %d]\n', 'AB');
fprintf('[%s]\n', 'AB');
fprintf('[%s %s]\n', 'ab', 'cd');
fprintf('[%s]=[%d]\n', 'x', 5);
