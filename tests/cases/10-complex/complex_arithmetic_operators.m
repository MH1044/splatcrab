% covers: 7 - (Scope, complex arithmetic) *, /, - and unary minus on complex scalars, a matrix product and .* with complex operands
z = (1+2i) * (3-1i); fprintf('%.4f %.4f\n', real(z), imag(z))
z = (5+5i) / (1+2i); fprintf('%.4f %.4f\n', real(z), imag(z))
z = (1+2i) - (4-3i); fprintf('%.4f %.4f\n', real(z), imag(z))
z = -(2-3i); fprintf('%.4f %.4f\n', real(z), imag(z))
z = [1+1i 2] * [3; 1i]; fprintf('%.4f %.4f\n', real(z), imag(z))
z = [1i 2] .* [3 1+1i]; fprintf('%.4f ', real(z), imag(z)); fprintf('\n')
