% covers: 6 - sum and mean along a dimension of size 1 within ndims return the argument, a -0 kept, and with no dimension the default one is judged the same way; sum past ndims, cumsum along a dimension of size 1 and the class of a sum of a logical are unchanged
% 1/x shows the sign of a zero: -Inf for -0, Inf for +0. The values are the
% spec's (S6 and Scope, sum and mean along a dimension of size 1).
disp(1/sum(-0))
disp(1/sum(-0, 1))
disp(1/mean(-0))
disp(1/mean(-0, 1))
disp(1 ./ sum([-0 -0], 1))
disp(1 ./ mean([-0; -0], 2))
x = 1 ./ sum(-0 * ones(1, 1, 2), 1); disp(x(:)')
disp(1/sum(-0, 3))
disp(1/cumsum(-0, 1))
disp(class(sum(true, 1)))
