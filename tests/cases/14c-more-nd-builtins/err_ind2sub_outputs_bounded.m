% covers: 11 - ind2sub's outputs are judged together before any is written, however many a program asks for: a hundred thousand outputs of a hundred thousand indices, asked for through eval, refused at once as one array too large; exit 1
s = ['[' repmat('a, ', 1, 99999) 'b] = ind2sub([2 3], 1:1e5);'];
eval(s)
