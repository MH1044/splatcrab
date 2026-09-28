% covers: 6 - func2str renders an anonymous function with no spaces around its operators and a named handle as its bare name; str2func makes a callable handle from text
disp(func2str(@(x) x.^2 + 1)); f = str2func('@(x) x*3'); disp(f(2)); disp(func2str(@sin))
