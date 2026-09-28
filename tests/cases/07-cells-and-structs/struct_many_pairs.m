% covers: Scope, the builtins - struct(name, value, ...) with many pairs is linear: 50,000
% pairs took 18.9 s before cycle 07's review fix, past the harness's timeout, and a field
% named twice still keeps its first place (checked below with a small struct)
n = 50000; c = cell(1, 2*n);
for k = 1:n, c{2*k-1} = sprintf('f%d', k); c{2*k} = k; end
s = struct(c{:});
disp(numel(fieldnames(s)))
disp(s.f50000)
t = struct('a', 1, 'b', 2, 'a', 3); f = fieldnames(t); disp(f{1}); disp(t.a)
