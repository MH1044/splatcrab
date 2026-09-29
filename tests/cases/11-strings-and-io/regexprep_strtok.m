% covers: 11 - regexprep replaces every match of \d, and strtok splits at the first whitespace, the remainder keeping its leading space
disp(regexprep('abc123', '\d', ''))
[tok, rest] = strtok('hello world');
disp(tok)
disp(rest)
