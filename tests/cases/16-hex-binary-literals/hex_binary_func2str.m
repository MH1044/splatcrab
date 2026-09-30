% covers: 3 - a negative literal is the negation of its magnitude in the parse tree, so func2str renders it with a negation's precedence and the text reads back as the same function
f = @() 0xFFs8^2;
disp(func2str(f))
g = str2func(func2str(f));
disp([f() g()])
h = @() 0x80s8.^2;
disp(func2str(h))
disp([h() feval(str2func(func2str(h)))])
r = @() -0xFFs8^2;
disp(func2str(r))
disp([r() feval(str2func(func2str(r)))])
