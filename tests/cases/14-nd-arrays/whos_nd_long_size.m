% covers: 11 - whos of an N-D array whose size text is longer than 65,535 characters writes it whole, with no limit on the size column's width
A = zeros([2 ones(1, 40000) 2]);
t = evalc('whos');
disp(numel(strfind(t, 'x1')) == 40000)
disp(any(strfind(t, 'double')))
