% covers: 11 - a struct whose field name is longer than 65,535 characters displays it whole, since no formatting width is fed by a length the program controls
s.(repmat('a', 1, 70000)) = 1;
t = evalc('disp(s)');
disp(numel(t) > 70000)
