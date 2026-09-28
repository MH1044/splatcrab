% covers: single and double quoted strings, doubled-quote escape, indexing, numeric use
% NOTE: "dq" is a string object in MATLAB. SplatCrab has no string class yet,
% NOTE: so it is a char here and displays as one.
s = 'hi'
d = "dq"
q = 'it''s'
s(1)
s(1:2)
'a' + 1
num2str(3.5)
num2str(7)
sprintf('%d-%s', 4, 'x')
disp('literal')
