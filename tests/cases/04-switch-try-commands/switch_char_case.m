% covers: 2 - switch on a char vector takes the case with equal text, and a number never matches a char case
s = 'abc'; switch s, case 'xyz', disp(1), case 'abc', disp(2), end; switch 5, case 'abc', disp(3), end
