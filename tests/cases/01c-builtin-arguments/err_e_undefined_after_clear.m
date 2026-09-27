% covers: 2 - once a variable named e is cleared, e is undefined again, not a constant
e = 5;
disp(e)
clear('e');
disp(e)
