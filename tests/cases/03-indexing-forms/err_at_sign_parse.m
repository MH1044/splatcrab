% covers: Scope, lexer { } . @ - the lexer's @ is a token: a bare @ is a parse error naming it, until cycle 06
% The rendering `unexpected '<token>' in expression` is pinned by
% 01e-display-and-parser/err_parse_token_semi; before this cycle the same
% input was the lexer's `unexpected character '@'`. No output: the script
% is parsed whole before it runs.
disp(1)
f = @
