% covers: 4 - char arithmetic gives double; indexed assignment into a char keeps it char
% s(2) = 'Z' used to turn s into the codes 97 90 99; it must stay the char aZc.
s = 'abc';
disp(s + 0)
s(2) = 'Z';
disp(s)
disp(class(s(2)))
