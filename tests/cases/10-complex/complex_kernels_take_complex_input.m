% covers: 12 - (Scope, the kernels that take complex input) sum, prod, mean and cumsum keep the imaginary part, and exp, sin and cos take a complex argument
s = sum([1+2i 3-1i]); fprintf('%.4f %.4f\n', real(s), imag(s))
p = prod([1+2i 3-1i]); fprintf('%.4f %.4f\n', real(p), imag(p))
m = mean([2+4i 4+2i]); fprintf('%.4f %.4f\n', real(m), imag(m))
c = cumsum([1i 2 3i]); fprintf('%.4f ', real(c), imag(c)); fprintf('\n')
disp(abs(exp(1i * pi) + 1) < 1e-12)
z = 1+2i; disp(abs(sin(z) - (exp(1i * z) - exp(-1i * z)) / 2i) < 1e-12)
disp(abs(cos(z) - (exp(1i * z) + exp(-1i * z)) / 2) < 1e-12)
