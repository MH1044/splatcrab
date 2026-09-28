% covers: 18 - a handle inside a cell literal evaluates, and c{1}(1) calls it
c = {@(x) x + 1, 2}; disp(c{1}(1)); disp(c{2})
