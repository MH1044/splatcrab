% covers: 1 - switch on a number takes the case whose cell array holds the value
x = 2; switch x, case 1, disp('one'), case {2, 3}, disp('two or three'), otherwise, disp('other'), end
