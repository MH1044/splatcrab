% covers: 9 - (Scope, deconv) deconv undoes conv: [1 5 6] divided by [1 2] is the quotient [1 3] and a zero remainder
% The remainder satisfies u = conv(v, q) + r, so it is as long as u. The
% long division is exact in binary, so both print as integers.
[q, r] = deconv([1 5 6], [1 2]); disp(q); disp(r)
