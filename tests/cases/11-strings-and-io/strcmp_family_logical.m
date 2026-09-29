% covers: 4 - strcmp and strcmpi return logicals, four wide in disp; strcmp of a cell compares each element; == compares chars element by element
disp(strcmp('a', 'a'))
disp(strcmp('a', 'b'))
disp(strcmpi('A', 'a'))
disp(strcmp({'a', 'b'}, 'a'))
disp('abc' == 'abd')
