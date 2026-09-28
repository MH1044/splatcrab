% covers: 1 - end_stack lives in the frame: in x(prev(end)) end is x's, prev's own w(end) is w's, and x's end survives the call
% x has 5 elements, so prev is passed 5. Inside prev, w(end) is 3, w's own
% length and not the caller's 5, so prev(5) is 5 - 3 + 2 = 4 and the first
% disp prints x(4). In the second, the end after the call is still x's 5,
% so the index is 4 + 5 - 4 = 5. Were the caller's end to leak into prev,
% prev(5) would be 2 and the first disp would print x(2), 20.
x = [10 20 30 40 50];
disp(x(prev(end)))
disp(x(prev(end) + end - 4))
function r = prev(k)
w = [1 2 3];
r = k - w(end) + 2;
end
