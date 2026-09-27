% covers: if/elseif/else, for over a range and over matrix columns, while, break, continue
A = [1 2; 3 4];
total = 0;
n = 0;
while total < 20
    n = n + 1;
    total = total + n;
end
fprintf('n = %d, total = %d\n', n, total);
if det(A) < 0
    disp('negative determinant')
elseif det(A) == 0
    disp('singular')
else
    disp('positive determinant')
end
s = 0;
for col = A
    s = s + col(1);
end
disp(s)
acc = 0;
for k = 1:10
    if k > 5
        break
    end
    if mod(k, 2) == 0
        continue
    end
    acc = acc + k;
end
disp(acc)
