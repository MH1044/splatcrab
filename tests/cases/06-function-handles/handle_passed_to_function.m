% covers: 13 - a handle passed as an argument is called inside the function through its parameter
r = apply(@(x) x * 2, 5); disp(r)
function r = apply(f, v)
r = f(v);
end
