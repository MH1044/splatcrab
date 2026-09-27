% covers: comparisons producing 0/1 arrays, elementwise and short-circuit logicals, not
x = [1 2 3] > 1
y = [1 0 1] & [1 1 0]
z = [1 0 0] | [0 0 1]
n = ~[1 0 2]
eqs = [1 2] == [1 3]
nes = [1 2] ~= [1 3]
if 3 > 2 && ~false
    disp('short circuit ok')
end
if false || true
    disp('or ok')
end
