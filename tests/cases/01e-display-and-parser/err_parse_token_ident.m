% covers: 3 - the same for an identifier: 'x1F', not Ident("x1F")
% 0x1F is a hex literal MATLAB R2019b and later accept and this lexer does
% not (a separate row, scheduled later); here it is just a convenient way to
% put an unexpected identifier in front of the parser.
x = 0x1F
