% covers: 15 - rearranging a char keeps it char; unary plus gives double (QA D17)
x = fliplr('abc');
disp(class(x))
disp(x)
disp(sort('cab'))
s = [];
s = [s 'abc'];
disp(class(s))
t = 'ab'.';
disp(size(t))
disp(class(t))
disp(class(+'a'))
