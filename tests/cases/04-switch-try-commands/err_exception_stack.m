% covers: 16 - e.stack waited for cycle 05 and then 07; now a struct array, empty for an error raised outside every function
try, error('x'), catch e, disp(class(e.stack)), disp(numel(e.stack)), end
