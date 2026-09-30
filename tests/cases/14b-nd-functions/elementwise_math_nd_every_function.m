% covers: 5 - every element-wise function taught N-D gives an N-D argument's shape and, element by element, what it gives for the same elements in a column, real or complex as today, for a real and for a complex argument; round with a digit count; and the two-argument mod, rem, atan2, hypot, power and complex broadcasting a column against an N-D array
X = reshape(-11:12, 2, 3, 4) / 10;
names = {'abs', 'sqrt', 'exp', 'log', 'log2', 'log10', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'sinh', 'cosh', 'tanh', 'floor', 'ceil', 'round', 'fix', 'sign', 'isnan', 'isinf', 'isfinite', 'real', 'imag', 'conj', 'angle'};
for k = 1:numel(names)
  f = names{k};
  Y = feval(f, X);
  c = feval(f, X(:));
  fprintf('%s %d %d %d %d\n', f, isequal(size(Y), [2 3 4]), isequal(Y(:), c), strcmp(class(Y), class(c)), isreal(Y));
end
Y = round(X * 7, 1);
disp(size(Y))
disp(isequal(Y(:), round(X(:) * 7, 1)))
A = reshape(1:24, 2, 3, 4);
E = [3; 4] .* ones(2, 3, 4);
twos = {'mod', 'rem', 'atan2', 'hypot', 'power', 'complex'};
for k = 1:numel(twos)
  f = twos{k};
  Y = feval(f, A, [3; 4]);
  fprintf('%s %d %d %d\n', f, isequal(size(Y), [2 3 4]), isequal(Y(:), feval(f, A(:), E(:))), isreal(Y));
end
Z = X + 1i * reshape(12:-1:-11, 2, 3, 4) / 20;
cnames = {'abs', 'sqrt', 'exp', 'log', 'log2', 'log10', 'sin', 'cos', 'asin', 'acos', 'real', 'imag', 'conj', 'angle'};
for k = 1:numel(cnames)
  f = cnames{k};
  Y = feval(f, Z);
  fprintf('%s %d %d %d\n', f, isequal(size(Y), [2 3 4]), isequal(Y(:), feval(f, Z(:))), isreal(Y));
end
