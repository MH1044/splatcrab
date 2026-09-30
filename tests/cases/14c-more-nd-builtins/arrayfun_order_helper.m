function y = arrayfun_order_helper(x)
% Helper for arrayfun_nd: prints each element it is called with, so the
% order of the calls shows. It has no covers line and no .out, so the
% harness never runs it as a case.
fprintf('%d ', x);
y = x;
end
