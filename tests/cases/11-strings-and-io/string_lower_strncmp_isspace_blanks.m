% covers: 4 - (Scope, the rest of the string functions) lower, strncmp and strncmpi, isspace, isletter and blanks
% The predicates return logicals, four wide in disp as item 4's strcmp is.
% The brackets show the three blanks.
disp(lower('AbC'))
disp(strncmp('abcd', 'abxy', 2))
disp(strncmp('abcd', 'abxy', 3))
disp(strncmpi('ABcd', 'abXY', 2))
disp(isspace('a b'))
disp(isletter('a1B'))
disp(['[' blanks(3) ']'])
