% covers: 10 - the colon computes its upper half from the right-hand end point, and linspace pins its last element
% Self-checking rather than pinned digits: what matters is that the last
% element is exactly the end point and that the vector is symmetric about its
% middle, not how the intermediate values print.
x = 0:0.1:0.3;
disp(x(end) == 0.3)
x = -1:0.01:1;
disp(all(x + fliplr(x) == 0))
y = linspace(0, 1, 7);
disp(y(end) == 1)
