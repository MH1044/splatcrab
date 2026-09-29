% covers: 12 - (Scope, the kernels that take complex input) sin and cos on the imaginary axis are sinh(y)*i and cosh(y) exactly, so an overflow gives Inf, never a NaN from 0 * Inf
z = sin(1000i); disp(real(z) == 0); disp(imag(z) == Inf)
disp(cos(1000i) == Inf); disp(isreal(cos(2i)))
disp(abs(sin(2i) - sinh(2) * 1i) < 1e-12)
