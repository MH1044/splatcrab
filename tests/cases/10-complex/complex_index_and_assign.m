% covers: 6 - (Scope, Matrix.im) indexing, indexed assignment, concatenation and deletion carry the imaginary part, column-major like the real part
z = [1+2i 3-4i 5i]; w = z(2); fprintf('%.4f %.4f\n', real(w), imag(w))
A = [1 2; 3 4]; A(2, 1) = 5i; fprintf('%.4f ', real(A)); fprintf('\n')
fprintf('%.4f ', imag(A)); fprintf('\n')
B = [z; 2 * z]; fprintf('%.4f ', imag(B)); fprintf('\n')
z(1) = []; fprintf('%.4f ', imag(z)); fprintf('\n')
