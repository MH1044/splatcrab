% covers: 1 - a size of 0 makes an empty N-D array wherever it stands, however large the other sizes, judged by the product of every size and answered at once
x = ones(2^20, 2^20, 0);
disp(size(x))
disp(isempty(x))
y = zeros(2^40, 2^40, 0);
disp(numel(y))
disp(ndims(y))
z = zeros(0, 2^40, 2^40);
disp(numel(z))
w = zeros(2^40, 0, 2^40);
disp(length(w))
