% covers: 4 - arrayfun calls a handle on each element, and on the matching elements of two arrays, and returns the array of results
disp(arrayfun(@(x) x * 2, [1 2 3])); disp(arrayfun(@(a, b) a * b, [1 2], [3 4]))
