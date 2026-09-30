% covers: 4 - reshape of an empty to an N-D empty whose other sizes are huge, the 0 last or first, counts the product as the 0 it is, at once
y = reshape([], 2^40, 2^40, 0);
disp(numel(y))
disp(ndims(y))
z = reshape(zeros(0, 3), 0, 2^40, 2^40);
disp(numel(z))
disp(isempty(z))
