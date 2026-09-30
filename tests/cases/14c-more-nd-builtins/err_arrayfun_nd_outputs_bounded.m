% covers: 12 - arrayfun of an N-D array judges the lists of sizes of its uniform outputs together, ndims by the outputs, before it calls f: a thousand and twenty-four outputs over an array of 2^20 dimensions are refused at once, exit 1
X = zeros([ones(1, 2^20 - 1) 2]);
eval(['[' repmat('a, ', 1, 1023) 'b] = arrayfun(@(x) deal(x), X);']);
disp('not reached')
