% covers: 2 - transposing or negating an empty array with a huge row count returns at once
x = zeros(1e12, 0); tic; y = x'; z = -x; disp(isempty(y) && isempty(z)); disp(toc < 2)
