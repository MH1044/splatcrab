% covers: 1 - an element-wise operation on an empty array with a huge column count, real or complex, returns at once
% Before 13b the broadcast loop ran once per column of the 0x1e12 operand.
x = zeros(0, 1e12); tic; y = x + 1; z = x .* 1i; w = x == 1i; v = power(x, 0.5); disp(isempty(y) && isempty(z) && isempty(w) && isempty(v)); disp(toc < 2)
