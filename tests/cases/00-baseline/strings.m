% covers: single and double quoted strings, doubled-quote escape, indexing, numeric use
% NOTE: MATLAB displays char arrays without quotes, and "dq" is a string object there.
% NOTE: SplatCrab has no string class yet; cycle 02 reworks char display.
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
