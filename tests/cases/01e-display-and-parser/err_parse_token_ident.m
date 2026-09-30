% covers: 3 - the same for an identifier: 'x1F', not Ident("x1F")
% 00x1F is the number 00 followed by the name x1F, a convenient way to put
% an unexpected identifier in front of the parser. It was 0x1F until cycle
% 16 made that a hexadecimal literal; a 0 before it keeps the old reading.
x = 00x1F
