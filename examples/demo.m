% MatLabTwo demo script
A = [1 2; 3 4]
B = A * A
A'
A .* A
x = A \ [5; 6]
inv(A)
det(A)

v = 1:5;
v(end)
v(2:4)
A(:, 1)
A(2, :)

% growth on indexed assignment
z = [];
for k = 1:4
    z(end+1) = k^2;
end
z

% control flow
total = 0;
n = 0;
while total < 20
    n = n + 1;
    total = total + n;
end
fprintf('n = %d, total = %d\n', n, total);

if det(A) < 0
    disp('A has a negative determinant')
else
    disp('A has a non-negative determinant')
end

s = sum(A(:))
m = mean([1 2 3 4])
pi
fprintf('%6.3f\n', linspace(0, 1, 3));
